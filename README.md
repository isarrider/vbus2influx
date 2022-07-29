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

I have not written the code myself, only stated what I need and a young engineer from work hacked it together for me<br>
and gave his blessing to put it up here under a permissive licence.<br>

What it does is it uses the library from Daniel Wippermann to dissect the data stream and<br>
a) displays this as raw content in a webserver<br>
b) pushes the data to InfluxDB

I included a dockerfile so it is (more) easy to deploy.

Proof that the Pi3 is overkill...
![Clipboard01](https://user-images.githubusercontent.com/6953309/181697857-e6a26a3e-ba0e-4dd4-9741-4b94376aa0f4.png)

A big thanks to Daniel wippermann for some really valueable hints and<br>
providing the library and to my coworker who wants to stay annonymous as he thinks his code is not "clean" enough ;)<br>

I'll update this when I have some Grafana screenshots
