# Run tests

```bash
pip install oh
oh &
# edge without reregister
cargo run
# edge | oneshot with reregister
cargo run --features "oneshot reregister2"
```

# CI log
* macOS & Linux: https://travis-ci.org/loggerhead/mio-udp-test
* windows: https://ci.appveyor.com/project/loggerhead/mio-udp-test
