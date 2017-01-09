# Run tests

```bash
pip install oh
oh &
# edge
cargo run
# level
cargo run --features "level"
# edge | oneshot with reregister
cargo run --features "oneshot reregister"
```

# CI log
* macOS & Linux: https://travis-ci.org/loggerhead/mio-udp-test
* windows: https://ci.appveyor.com/project/loggerhead/mio-udp-test
