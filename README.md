# Description

Websocket client that downloads KataGo v1.13.0 binary and one of the latest models, runs it and communicates with websocket using the process stdin and stdout.

Katago is run using this args:
```console
analysis
-model kata1-b18c384nbt-s8341979392-d3881113763.bin.gz
-config KataGo/configs/analysis_example.cfg
```
# Example usage

```console
cargo run ws://127.0.0.1:4000
```

# Requirements

cargo

# Debian/Ubuntu package requirements

all versions
```
zlib1g-dev
libzip-dev
```

GPU (OpenCL) version
```
ocl-icd-opencl-dev
mesa-opencl-icd
```