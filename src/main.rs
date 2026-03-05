use std::{
    collections::VecDeque,
    io::{self, Read},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use axum::{routing::get, Json, Router};
use color_eyre::{eyre::eyre, Result};
use figment::{
    providers::{Format, Toml},
    Figment,
};
use futures::stream;
use influxdb2::{models::DataPoint, Client};
use resol_vbus::{
    chrono::{DateTime, Utc},
    Data, DataSet, Language, LiveDataReader, Specification, SpecificationFile,
};
use rppal::{
    gpio,
    uart::{self, Parity, Uart},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Deserialize)]
struct Config {
    db_url: String,
    db_token: String,
    db_org: String,
    db_bucket: String,
    db_measurement: String,
    uart_path: PathBuf,
    webserver_address: Option<SocketAddr>,
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    // Load config file
    let config: Config = Figment::new()
        .merge(Toml::file("/etc/vbus2influx.toml"))
        .extract()?;
    let config = Arc::new(config);

    // Create InfluxDB Client — bucket is no longer part of the client,
    // it is passed per write call instead.
    let client = Client::new(&config.db_url, &config.db_org, &config.db_token);

    let measurements = Arc::new(Mutex::new(Measurements::zeroed()));

    if config.webserver_address.is_some() {
        tokio::spawn(run_webserver(
            Arc::clone(&config),
            Arc::clone(&measurements),
        ));
    }

    // Include specification in binary and decode it at runtime
    let spec_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/vbus_specification.vsf",
    ));
    let spec_file = SpecificationFile::from_bytes(spec_bytes)?;
    let spec = Specification::from_file(spec_file, Language::En);

    // Read data from UART
    let uart = Uart::with_path(&config.uart_path, 9600, Parity::None, 8, 1)?;
    let mut data_reader = LiveDataReader::new(0, UartWrapper(uart));

    let mut measurement_buffer: VecDeque<Measurements> = VecDeque::new();
    loop {
        let current_measurements = read_data(&mut data_reader, &spec)?;
//        println!("Received Measurements: {:?}", measurements);
        *measurements.lock().await = current_measurements.clone();
        measurement_buffer.push_back(current_measurements);

        while let Some(m) = measurement_buffer.pop_front() {
            // Build a DataPoint from the measurement struct
            let point = DataPoint::builder(&config.db_measurement)
                .timestamp(m.time.timestamp_nanos())
                .field("temperature_01", m.temperature_01)
                .field("temperature_02", m.temperature_02)
                .field("temperature_03", m.temperature_03)
                .field("temperature_04", m.temperature_04)
                .field("temperature_05", m.temperature_05)
                .field("temperature_06", m.temperature_06)
                .field("temperature_07", m.temperature_07)
                .field("temperature_08", m.temperature_08)
                .field("temperature_09", m.temperature_09)
                .field("irradiation_10", m.irradiation_10)
                .field("temperature_11", m.temperature_11)
                .field("temperature_12", m.temperature_12)
                .field("flow_rate_09", m.flow_rate_09)
                .field("flow_rate_11", m.flow_rate_11)
                .field("flow_rate_12", m.flow_rate_12)
                .field("pressure_11", m.pressure_11)
                .field("pressure_12", m.pressure_12)
                .field("relay_01", m.relay_01)
                .field("relay_02", m.relay_02)
                .field("relay_03", m.relay_03)
                .field("relay_04", m.relay_04)
                .field("relay_05", m.relay_05)
                .field("PWM_0_10V_A", m.PWM_0_10V_A)
                .field("PWM_0_10V_B", m.PWM_0_10V_B)
                .build();

            match point {
                Ok(point) => {
                    // Write measurements to InfluxDB
                    let res = client
                        .write(&config.db_bucket, stream::iter(vec![point]))
                        .await;

                    if let Err(err) = res {
                        eprintln!("Error while sending data to InfluxDB: {err}");
                        measurement_buffer.push_front(m);
                        break;
                    }
                }
                Err(err) => {
                    eprintln!("Error while building DataPoint: {err}");
                }
            }
        }
    }
}

struct UartWrapper(Uart);

impl Read for UartWrapper {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0
            .set_read_mode(buf.len().try_into().unwrap_or(u8::MAX), Duration::ZERO)
            .map_err(uart_err_to_io)?;
        self.0.read(buf).map_err(uart_err_to_io)
    }
}

fn uart_err_to_io(err: uart::Error) -> io::Error {
    match err {
        uart::Error::Io(err) => err,
        uart::Error::Gpio(gpio::Error::Io(err)) => err,
        uart::Error::Gpio(err) => io::Error::new(io::ErrorKind::Other, err),
        uart::Error::InvalidValue => io::Error::new(io::ErrorKind::InvalidInput, err),
    }
}

async fn run_webserver(config: Arc<Config>, measurements: Arc<Mutex<Measurements>>) -> Result<()> {
    let app = Router::new().route(
        "/",
        get(move || async move {
            let measurements = measurements.lock().await.clone();
            Json(measurements)
        }),
    );
    axum::Server::bind(config.webserver_address.as_ref().unwrap())
        .serve(app.into_make_service())
        .await?;
    Ok(())
}


/// Reads measurements from live vbus data.
fn read_data<R: Read>(
    reader: &mut LiveDataReader<R>,
    spec: &Specification,
) -> Result<Measurements> {
    // Read data into dataset
    let mut dataset = DataSet::new();
    while let Some(data) = reader.read_data()? {
        match &data {
            Data::Packet(packet)
                if packet.command == 0x0100 && packet.header.destination_address == 0x0010 =>
            {
                dataset.add_data(data);
                break;
            }
            _ => {}
        }
    }
    // Get fields from dataset
    let mut fields = spec.fields_in_data_set(&dataset);
    let measurements = Measurements {
        time: Utc::now(),
        temperature_01: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_01` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_01` can't be converted to `f64`."))?,
        temperature_02: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_02` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_02` can't be converted to `f64`."))?,
        temperature_03: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_03` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_03` can't be converted to `f64`."))?,
        temperature_04: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_04` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_04` can't be converted to `f64`."))?,
        temperature_05: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_05` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_05` can't be converted to `f64`."))?,
        temperature_06: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_06` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_06` can't be converted to `f64`."))?,
        temperature_07: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_07` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_07` can't be converted to `f64`."))?,
        temperature_08: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_08` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_08` can't be converted to `f64`."))?,
        temperature_09: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_09` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_09` can't be converted to `f64`."))?,
        irradiation_10: fields
            .next()
            .ok_or_else(|| eyre!("Field `irradiation_10` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `irradiation_10` can't be converted to `f64`."))?,
        temperature_11: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_11` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_11` can't be converted to `f64`."))?,
        temperature_12: fields
            .next()
            .ok_or_else(|| eyre!("Field `temperature_12` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `temperature_12` can't be converted to `f64`."))?,
        flow_rate_09: fields
            .next()
            .ok_or_else(|| eyre!("Field `flow_rate_09` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `flow_rate_09` can't be converted to `f64`."))?,
        flow_rate_11: fields
            .next()
            .ok_or_else(|| eyre!("Field `flow_rate_11` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `flow_rate_11` can't be converted to `f64`."))?,
        flow_rate_12: fields
            .next()
            .ok_or_else(|| eyre!("Field `flow_rate_12` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `flow_rate_12` can't be converted to `f64`."))?,
        pressure_11: fields
            .next()
            .ok_or_else(|| eyre!("Field `pressure_11` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `pressure_11` can't be converted to `f64`."))?,
        pressure_12: fields
            .next()
            .ok_or_else(|| eyre!("Field `pressure_12` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `pressure_12` can't be converted to `f64`."))?,
        relay_01: fields
            .next()
            .ok_or_else(|| eyre!("Field `relay_01` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `relay_01` can't be converted to `f64`."))?,
        relay_02: fields
            .next()
            .ok_or_else(|| eyre!("Field `relay_02` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `relay_02` can't be converted to `f64`."))?,
        relay_03: fields
            .next()
            .ok_or_else(|| eyre!("Field `relay_03` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `relay_03` can't be converted to `f64`."))?,
        relay_04: fields
            .next()
            .ok_or_else(|| eyre!("Field `relay_04` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `relay_04` can't be converted to `f64`."))?,
        relay_05: fields
            .next()
            .ok_or_else(|| eyre!("Field `relay_05` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `relay_05` can't be converted to `f64`."))?,
        PWM_0_10V_A: fields
            .next()
            .ok_or_else(|| eyre!("Field `PWM_0_10V_A` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `PWM_0_10V_A` can't be converted to `f64`."))?,
        PWM_0_10V_B: fields
            .next()
            .ok_or_else(|| eyre!("Field `PWM_0_10V_B` not set."))?
            .raw_value_f64()
            .ok_or_else(|| eyre!("Field `PWM_0_10V_B` can't be converted to `f64`."))?,
    };

    Ok(measurements)
}

// InfluxDbWriteable derive removed — fields are written via DataPoint::builder above
#[derive(Debug, Clone, Serialize)]
struct Measurements {
    time: DateTime<Utc>,
    temperature_01: f64,
    temperature_02: f64,
    temperature_03: f64,
    temperature_04: f64,
    temperature_05: f64,
    temperature_06: f64,
    temperature_07: f64,
    temperature_08: f64,
    temperature_09: f64,
    irradiation_10: f64,
    temperature_11: f64,
    temperature_12: f64,
    flow_rate_09: f64,
    flow_rate_11: f64,
    flow_rate_12: f64,
    pressure_11: f64,
    pressure_12: f64,
    relay_01: f64,
    relay_02: f64,
    relay_03: f64,
    relay_04: f64,
    relay_05: f64,
    PWM_0_10V_A: f64,
    PWM_0_10V_B: f64,
}

impl Measurements {
    fn zeroed() -> Self {
        Measurements {
            time: Utc::now(),
            temperature_01: 0.0,
            temperature_02: 0.0,
            temperature_03: 0.0,
            temperature_04: 0.0,
            temperature_05: 0.0,
            temperature_06: 0.0,
            temperature_07: 0.0,
            temperature_08: 0.0,
            temperature_09: 0.0,
            irradiation_10: 0.0,
            temperature_11: 0.0,
            temperature_12: 0.0,
            flow_rate_09: 0.0,
            flow_rate_11: 0.0,
            flow_rate_12: 0.0,
            pressure_11: 0.0,
            pressure_12: 0.0,
            relay_01: 0.0,
            relay_02: 0.0,
            relay_03: 0.0,
            relay_04: 0.0,
            relay_05: 0.0,
            PWM_0_10V_A: 0.0,
            PWM_0_10V_B: 0.0,
        }
    }
}
