<img src="https://github.com/b-camacho/canopeners/assets/12277070/5314c727-6eb5-41b3-92a5-2b2abaa504c3" width="400">

# ⚠️ WIP ⚠️
It doesn't work quite yet, but soon it will!

# canopeners
Incomplete, but easy to use implementation of the CANOpen standard in Rust.

# Examples
All examples are blocking. Set timeouts with `conn.set_{read_write}_timeout`.

Send a single message:
```rust
Conn::new("vcan0").map(|conn| {
    let nmt = Nmt::new(canopeners::NmtFunction::StartRemoteNode, 10);
    conn.send(&Message::Nmt(nmt)).unwrap();
})
```

Write bytes to object dictionary on a remote node:
```rust
Conn::new("vcan0").map(|mut conn| {
    conn.sdo_write(
    /* remote node id */ 0x10, 
    /* index */ 0x1000,
    /* sub index */ 1,
    /* data, can be any length */ &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
    .unwrap();
})
```

Read bytes from object dictionary on a remote node:
```rust
Conn::new("vcan0").map(|mut conn| {
    let res = conn.sdo_read(
    /* remote node id */ 0x10, 
    /* index */ 0x1000,
    /* sub index */ 1)
    .unwrap();
    dbg!(res);
})
```


# Building
```
nix develop
cargo build
```
if you'd rather use your system cargo, just `cargo build` will work too

# Testing
`setup_vcan.sh` sets up a virtual CAN bus. `tests/` rely on this



# TODO
- [x] enum for all message types (can't use impl trait as function return type)
- [x] send/receive SDO
- [x] stateless example over vcan
- [x] segmented SDO
- [x] porcelain wrappers for easy send/receive over SDO
- [ ] finish replacing manual bit manipulation with binrw
- [x] package.nix
- [x] convert simple.rs example into tests
- [x] fix cargo warns
- [x] fix clippy lints
- [ ] add `send_acked` for all message types
- [ ] Node impl sending TPDOs based on SYNC msgs
- [ ] extended ID support

