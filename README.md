# vbus2influx

Plan was to be able to have a Grafana panel, where I can see the 8 different temperature sensors + 1 Pump speed (in %).<br>

As I have a Resol DeltaSol BX Plus, there is a bus called VBus putting out this data without the need to poll.<br>

# Hardware

So, with a tiny circuit it is possible to listen to the VBus.<br>
Circuitry is not my work and can be found online, but I modified it anyway...<br>
(No R3, the voltage divider R1 and R2 brings the voltage down to less then 2,5V to be safe<br>
as I measured Voltages close to 9V directly on the vBus).<br>
What it does in the end is to shift voltage down and uses the transistors as switches to pull RX to ground or leave it on 3V3.<br>

Total cost is somewhat around 1-2€.

![Resol_VBus_adaptor_to_WemosD1Mini](https://user-images.githubusercontent.com/6953309/181694190-ed17f850-7d52-4fff-897e-6f5f72776b70.png)

some quick soldering and it looks like that:

![circuitry](https://user-images.githubusercontent.com/6953309/181695276-468818aa-a619-4abc-9a7b-62f771904203.jpg)

Instead of the ESP32 I attached it to a Pi3 (actually overkill)...

# Software

I have not written the code myself, only stated what I need and a young engineer from work hacked it together for me and gave his blessing to put it up here under a permissive licence.<br>

What it does is it uses the library from Daniel Wippermann to dissect the data stream and<br>
a) displays this as raw content in a webserver<br>
b) pushes the data to InfluxDB

# Docker

I included a dockerfile so it is (more) easy to deploy.<br>
It builds for amd64, arm64, arm/v7, arm/v6 (e.g. Raspberry Pi with a 64-bit or 32-bit OS) and riscv64. The binary is cross-compiled, so building for another architecture needs no emulation.

Build for the machine you're on:

    docker build -t vbus2influx .

Build for a Raspberry Pi from another machine and load it into the local Docker:

    docker buildx build --platform linux/arm64 -t vbus2influx --load .

Build all architectures as one multi-arch image and push it to a registry:

    docker buildx build --platform linux/amd64,linux/arm64,linux/arm/v7,linux/arm/v6,linux/riscv64 \
      -t <registry>/vbus2influx:latest --push .

I run it like:

docker run -d --restart unless-stopped \
 -p 11110:11110 \
 -v vbus2influx:/etc \
 --device /dev/ttyAMA0:/dev/ttyAMA0 \
 --name vbus2influx \
 -h vbus2influx \
 vbus2influx

(the vbus2influx.toml is to be placed in /etc)

# Breaking changes

## InfluxDB client switched to `influxdb` 0.8 (crates.io)

The InfluxDB client is now the [`influxdb`](https://crates.io/crates/influxdb) crate from crates.io instead of a git fork. Data is still written to `<db_url>/write` with the token, but the config settings behave differently:

| Setting | Before | Now |
|---|---|---|
| `db_org` | sent as `org` | **ignored**, can be removed |
| `db_bucket` | sent as `bucket` | sent as `db`, **optional** |
| `db_token` | required | **optional**, only sent when set |

**VictoriaMetrics:** set `db_bucket = ""` (or remove the line) to keep everything as it was.<br>
VictoriaMetrics ignored the old `bucket` parameter, but turns `db` into a label. With `db_bucket` set, every series gets a `db="<db_bucket>"` label, e.g. `vbus2influx_temperature_01{db="mybucket"}`. VictoriaMetrics treats that as a new series, so dashboards would show the old and the new data as separate lines.

**InfluxDB 2.x:** set `db_bucket` to the bucket name and `db_token` to a token with write access. The bucket needs a DBRP mapping for the 1.x `/write` endpoint. InfluxDB 2.10 and newer provides one automatically (virtual DBRP), older versions need `influx v1 dbrp create`.

# misc

Proof that the Pi3 is overkill...

![Clipboard01](https://user-images.githubusercontent.com/6953309/181697857-e6a26a3e-ba0e-4dd4-9741-4b94376aa0f4.png)

A big thanks to Daniel Wippermann for some really valueable hints and providing the library<br>
and to my coworker who wants to stay annonymous as he thinks his code is not "clean" enough ;)<br>

# Final Result

![image](https://user-images.githubusercontent.com/6953309/182209261-9085c7d8-ca47-4b35-ab78-62c18acd50f5.png)
