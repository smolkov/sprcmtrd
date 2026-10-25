# Quick start

Open the viewer in a directory with recordings; the file picker lists them:

```sh
sprcmtrd /data/recorder
```

Type a part of the file name to filter, `Enter` opens it. `?` shows all keys,
`q` quits. More in [Interactive viewer](viewer/index.md).

A first look from the command line:

```sh
sprcmtrd list /data/recorder --since 2026-10-01   # what is there
sprcmtrd check /data/recorder                     # anything broken?
sprcmtrd info fault.cfg                           # station, channels, rates
sprcmtrd events fault.cfg                         # start, trip, breaker open
sprcmtrd phasors fault.cfg --at 40                # values 40 ms after the trigger
sprcmtrd trend fault.cfg --channels IA,IB,IC      # the same cycle by cycle
```

`FILE` can be any file of a recording (`fault.cfg`, `fault.dat`, `fault.cff`);
the other files are found by name. All commands are listed in
[Commands](commands/index.md).
