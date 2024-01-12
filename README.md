<img src="https://github.com/b-camacho/canopeners/assets/12277070/5314c727-6eb5-41b3-92a5-2b2abaa504c3" width="400">

# ⚠️ WIP ⚠️
It doesn't work quite yet, but soon it will!

# canopeners
Incomplete, but easy to use implementation of the CANOpen standard in Rust.

# building
`nix develop`
`cargo build`

# TODO
- [x] enum for all message types (can't use impl trait as function return type)
- [x] send/receive SDO
- [x] stateless example over vcan
- [x] segmented SDO
- [x] porcelain wrappers for easy send/receive over SDO
- [ ] finish replacing manual bit manipulation with binrw
- [ ] package.nix
- [ ] convert simple.rs example into tests
- [ ] fix cargo warns
- [ ] fix clippy lints
- [ ] Node impl sending TPDOs based on SYNC msgs

