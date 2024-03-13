# localsend-rs

CLI implementation of [localsend](https://github.com/localsend/localsend).

## Install

```bash
$ cargo install --git https://github.com/zpp0196/localsend-rs.git
```

## Usage

### Send

```bash
# send text only
$ localsend send "text to sent"

# send files
$ localsend send /path/to/file1 /path/to/file2 ...

# send mixed texts and files
$ localsend send "text to sent" /path/to/file ...
```

### Receive

```bash
# receive files and save to $(pwd)
$ localsend receive

# receive files and save to path
$ localsend receive --dest /path/to/save

# receive all files automatically
$ localsend receive --quick-save
```

## Roadmap

- [x] Settings
    - [x] Device alias
    - [x] Device fingerprint
    - [x] Multicast address
    - [x] Port
    - [ ] Enable https
    - [x] Quick Save
    - [x] Save directory
    - [ ] Non interactive mode
- [x] Discovery
    - [x] Multicast UDP
    - [ ] ~~HTTP(Legacy Mode)~~
- [x] File transfer
    - [x] Send files and texts
    - [ ] Send clipboard data
    - [x] Cancel sending
    - [x] File upload progress bar
    - [x] Fuzzy Select devices
    - [x] Receive files
- [ ] Reverse file transfer
    - [ ] Browser URL
    - [ ] ~~Receive request~~(not in plan)

## Thanks

* [localsend/localsend](https://github.com/localsend/localsend) [#11](https://github.com/localsend/localsend/issues/11)
* [localsend/protocol](https://github.com/localsend/protocol)
* [notjedi/localsend-rs](https://github.com/notjedi/localsend-rs)
