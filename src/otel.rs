use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
    time::Duration,
};

use opentelemetry::{
    metrics::MeterProvider as _, trace::TracerProvider as _, InstrumentationScopeBuilder, KeyValue,
};
use opentelemetry_otlp::{
    Compression, ExporterBuildError, HasExportConfig, HasHttpConfig, MetricExporterBuilder,
    Protocol, SpanExporterBuilder, WithExportConfig, WithHttpConfig,
};
use opentelemetry_sdk::{
    metrics::{MeterProviderBuilder, PeriodicReader, PeriodicReaderBuilder},
    resource::ResourceBuilder,
    trace::TracerProviderBuilder,
};
#[cfg(feature = "schemars1")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::*;

use crate::ParseError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct Tracer {
    #[serde(default)]
    pub scope: InstrumentationScope,
    #[serde(default)]
    pub provider: TracerProvider,
}

impl Tracer {
    pub fn build(self) -> Result<opentelemetry_sdk::trace::Tracer, ExporterBuildError> {
        let Self { provider, scope } = self;
        provider
            .builder()
            .map(|b| b.build().tracer_with_scope(scope.builder().build()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct InstrumentationScope {
    pub name: String,
    pub version: Option<String>,
    pub schema_url: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
}

impl InstrumentationScope {
    pub fn builder(self) -> InstrumentationScopeBuilder {
        let Self {
            name,
            version,
            schema_url,
            attributes,
        } = self;
        let b = opentelemetry::InstrumentationScope::builder(name);
        let b = apply(b, version, |b, v| b.with_version(v));
        let b = apply(b, schema_url, |b, v| b.with_schema_url(v));
        b.with_attributes(attributes.into_iter().map(|(k, v)| KeyValue::new(k, v)))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct TracerProvider {
    #[serde(default)]
    pub resource: Resource,
    #[serde(default)]
    pub sampler: Sampler,
    #[serde(default)]
    pub exporters: Vec<SpanExporter>,

    pub max_events_per_span: Option<u32>,
    pub max_attributes_per_span: Option<u32>,
    pub max_links_per_span: Option<u32>,
    pub max_attributes_per_event: Option<u32>,
    pub max_attributes_per_link: Option<u32>,
}

impl TracerProvider {
    pub fn builder(self) -> Result<TracerProviderBuilder, ExporterBuildError> {
        let Self {
            exporters,
            sampler,
            max_events_per_span,
            max_attributes_per_span,
            max_links_per_span,
            max_attributes_per_event,
            max_attributes_per_link,
            resource,
        } = self;

        let b = TracerProviderBuilder::default();

        let b = apply(b, max_events_per_span, |b, v| b.with_max_events_per_span(v));
        let b = apply(b, max_attributes_per_span, |b, v| {
            b.with_max_attributes_per_span(v)
        });
        let b = apply(b, max_links_per_span, |b, v| b.with_max_links_per_span(v));
        let b = apply(b, max_attributes_per_event, |b, v| {
            b.with_max_attributes_per_event(v)
        });
        let b = apply(b, max_attributes_per_link, |b, v| {
            b.with_max_attributes_per_link(v)
        });

        let b = b.with_resource(resource.builder().build());

        Ok(exporters
            .into_iter()
            .try_fold(
                b,
                |acc,
                 SpanExporter {
                     batch,
                     http,
                     export,
                 }| {
                    export
                        .apply(http.apply(SpanExporterBuilder::new().with_http()))
                        .build()
                        .map(|exporter| match batch.unwrap_or(true) {
                            true => acc.with_batch_exporter(exporter),
                            false => acc.with_simple_exporter(exporter),
                        })
                },
            )?
            .with_sampler(match sampler {
                Sampler::Always => opentelemetry_sdk::trace::Sampler::AlwaysOn,
                Sampler::Never => opentelemetry_sdk::trace::Sampler::AlwaysOff,
                Sampler::Ratio(it) => opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(it),
            }))
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, PartialOrd, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct Resource {
    #[serde(default)]
    pub attributes: BTreeMap<String, Value>,
    pub schema_url: Option<String>,
    pub detect: Option<bool>,
}

impl Resource {
    pub fn builder(self) -> ResourceBuilder {
        let Self {
            attributes,
            schema_url,
            detect,
        } = self;
        apply(
            match detect.unwrap_or(true) {
                true => opentelemetry_sdk::Resource::builder(),
                false => opentelemetry_sdk::Resource::builder_empty(),
            },
            schema_url,
            |it, url| it.with_schema_url([], url),
        )
        .with_attributes(attributes.into_iter().map(|(k, v)| KeyValue::new(k, v)))
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(untagged)]
pub enum Value {
    Bool(bool),
    I64(i64),
    F64(f64),
    String(String),
    Array(Array),
}

impl From<Value> for opentelemetry::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Bool(it) => Self::Bool(it),
            Value::I64(it) => Self::I64(it),
            Value::F64(it) => Self::F64(it),
            Value::String(it) => Self::String(it.into()),
            Value::Array(it) => Self::Array(it.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(untagged)]
pub enum Array {
    Bool(Vec<bool>),
    I64(Vec<i64>),
    F64(Vec<f64>),
    String(Vec<String>),
}

impl From<Array> for opentelemetry::Array {
    fn from(value: Array) -> Self {
        match value {
            Array::Bool(items) => Self::Bool(items),
            Array::I64(items) => Self::I64(items),
            Array::F64(items) => Self::F64(items),
            Array::String(items) => Self::String(items.into_iter().map(Into::into).collect()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum Sampler {
    #[default]
    Always,
    Never,
    Ratio(f64),
}

impl Sampler {
    const PARSE_ERROR: &str = "Expected one of `always`, `never` or `ratio=<float>`";
}

impl FromStr for Sampler {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "always" => Self::Always,
            "never" => Self::Never,
            _ => match s.strip_prefix("ratio=").and_then(|s| s.parse().ok()) {
                Some(it) => Self::Ratio(it),
                None => return Err(ParseError(Self::PARSE_ERROR)),
            },
        })
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct SpanExporter {
    #[serde(default)]
    pub http: HttpConfig,
    #[serde(default)]
    pub export: ExportConfig,
    pub batch: Option<bool>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct HttpConfig {
    #[serde(default, with = "As::<Option<FromInto<_Compression>>>")]
    #[cfg_attr(feature = "schemars1", schemars(with = "Option<_Compression>"))]
    pub compression: Option<Compression>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl HttpConfig {
    pub fn apply<T: HasHttpConfig>(self, to: T) -> T {
        let Self {
            compression,
            headers,
        } = self;
        match compression {
            Some(it) => to.with_compression(it),
            None => to,
        }
        .with_headers(headers)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, Eq, PartialEq, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct ExportConfig {
    pub endpoint: Option<String>,
    pub timeout: Option<Duration>,

    #[serde(default, with = "As::<Option<FromInto<_Protocol>>>")]
    #[cfg_attr(feature = "schemars1", schemars(with = "Option<_Protocol>"))]
    pub protocol: Option<Protocol>,
}

impl ExportConfig {
    pub fn apply<T: HasExportConfig>(self, mut to: T) -> T {
        let Self {
            endpoint,
            protocol,
            timeout,
        } = self;
        let protocol = protocol.unwrap_or(to.export_config().protocol);
        to.with_export_config(opentelemetry_otlp::ExportConfig {
            endpoint,
            protocol,
            timeout,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct Meter {
    #[serde(default)]
    pub scope: InstrumentationScope,
    #[serde(default)]
    pub provider: MeterProvider,
}

impl Meter {
    pub fn build(self) -> Result<opentelemetry::metrics::Meter, ExporterBuildError> {
        let Self { scope, provider } = self;
        provider
            .builder()
            .map(|b| b.build().meter_with_scope(scope.builder().build()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct MetricExporter {
    pub http: HttpConfig,
    pub export: ExportConfig,
    pub temporality: Option<Temporality>,
    pub interval: Option<Duration>,
}

impl MetricExporter {
    pub fn builder(
        self,
    ) -> Result<PeriodicReaderBuilder<opentelemetry_otlp::MetricExporter>, ExporterBuildError> {
        let Self {
            temporality,
            http,
            export,
            interval,
        } = self;
        Ok(apply(
            PeriodicReader::builder(
                apply(
                    export.apply(http.apply(MetricExporterBuilder::new().with_http())),
                    temporality.map(Into::into),
                    MetricExporterBuilder::with_temporality,
                )
                .build()?,
            ),
            interval,
            |it, dur| it.with_interval(dur),
        ))
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Default)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub struct MeterProvider {
    #[serde(default)]
    pub exporters: Vec<MetricExporter>,
    #[serde(default)]
    pub resource: Resource,
}

impl MeterProvider {
    pub fn builder(self) -> Result<MeterProviderBuilder, ExporterBuildError> {
        let Self {
            exporters,
            resource,
        } = self;
        exporters.into_iter().try_fold(
            MeterProviderBuilder::default().with_resource(resource.builder().build()),
            |acc, el| el.builder().map(|it| acc.with_reader(it.build())),
        )
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
pub enum Temporality {
    Cumulative,
    Delta,
    LowMemory,
}

impl From<Temporality> for opentelemetry_sdk::metrics::Temporality {
    fn from(value: Temporality) -> Self {
        match value {
            Temporality::Cumulative => Self::Cumulative,
            Temporality::Delta => Self::Delta,
            Temporality::LowMemory => Self::LowMemory,
        }
    }
}

macro_rules! conv_enum {
    (
        #[convert($ty:ty)]
        $(#[$enum_meta:meta])*
        $enum_vis:vis enum $enum_name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant_name:ident
            ),* $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        $enum_vis enum $enum_name {
            $(
                $(#[$variant_meta])*
                $variant_name,
            )*
        }
        impl From<$ty> for $enum_name {
            fn from(value: $ty) -> Self {
                match value {
                    $(
                        <$ty>::$variant_name => Self::$variant_name,
                    )*
                }
            }
        }
        impl From<$enum_name> for $ty {
            fn from(value: $enum_name) -> Self {
                match value {
                    $(
                        $enum_name::$variant_name => Self::$variant_name,
                    )*
                }
            }
        }
    };
}

conv_enum! {
#[convert(Protocol)]
#[derive(Deserialize, Serialize, Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
enum _Protocol {
    Grpc,
    HttpBinary,
    HttpJson,
}}

conv_enum! {
#[convert(Compression)]
#[derive(Deserialize, Serialize, Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "schemars1", derive(JsonSchema))]
#[serde(rename_all = "kebab-case")]
enum _Compression {
    Gzip,
    Zstd,
}}

fn apply<T, V>(t: T, v: Option<V>, f: impl FnOnce(T, V) -> T) -> T {
    match v {
        Some(v) => f(t, v),
        None => t,
    }
}
