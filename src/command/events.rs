//! `events` command: timeline of binary channel changes.

use anyhow::bail;
use chrono::{NaiveDateTime, TimeDelta};
use serde::Serialize;

use super::info::key_values;
use super::{Context, select_channels, table, time, time_window};
use crate::args::EventsArgs;
use crate::comtrade::Comtrade;
use crate::comtrade::events::{self, Event};

#[derive(Serialize)]
struct JsonEvent<'a> {
    #[serde(flatten)]
    event: &'a Event,
    /// Date and time of day of the change.
    absolute: String,
}

#[derive(Serialize)]
struct Json<'a> {
    file: String,
    trigger_time: NaiveDateTime,
    /// Window start in s relative to the trigger.
    from_s: f64,
    /// Window end in s relative to the trigger.
    to_s: f64,
    events: Vec<JsonEvent<'a>>,
}

impl EventsArgs {
    /// Runs the `events` command.
    pub fn run(self, _ctx: &Context) -> anyhow::Result<()> {
        let Self {
            file,
            channels,
            from: from_ms,
            to: to_ms,
            json,
        } = self;
        let c = Comtrade::open(&file)?;
        let (analog, binary) = select_channels(&c, &channels)?;
        // No selection means all channels, analog ones included; only a named
        // analog channel is a mistake.
        if let Some(&i) = analog.first()
            && !channels.is_empty()
        {
            let ch = c.analog(i).expect("selected index is valid");
            bail!("{} is an analog channel; events lists binary channels", ch.info.id);
        }
        let binary: Vec<_> = binary
            .iter()
            .map(|&i| c.binary(i).expect("selected index is valid"))
            .collect();
        let (t0, t1) = time_window(&c, from_ms, to_ms)?;
        let range = c.index_range(t0, t1);
        let list = events::events(&binary, c.time(), range.clone());
        let trigger = c.config.trigger_time;
        let absolute = |t: f64| {
            (trigger + TimeDelta::nanoseconds((t * 1e9).round() as i64))
                .format("%Y-%m-%d %H:%M:%S%.6f")
                .to_string()
        };
        let name = c
            .path
            .as_ref()
            .map(|p| p.main_file().display().to_string())
            .unwrap_or_default();

        if json {
            let out = Json {
                file: name,
                trigger_time: trigger,
                from_s: t0,
                to_s: t1,
                events: list
                    .iter()
                    .map(|e| JsonEvent {
                        event: e,
                        absolute: absolute(e.time),
                    })
                    .collect(),
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
            return Ok(());
        }

        let span = t1 - t0;
        let signed = |t: f64| {
            let s = time(t, span);
            if t > 0.0 { format!("+{s}") } else { s }
        };
        let mut out = key_values(&[
            ("File", name),
            (
                "Window",
                format!("{} .. {} ({} samples)", signed(t0), signed(t1), range.len()),
            ),
            ("Trigger", trigger.format("%Y-%m-%d %H:%M:%S%.6f").to_string()),
        ]);
        if binary.is_empty() {
            out += "\nThe recording has no binary channels.\n";
            print!("{out}");
            return Ok(());
        }
        if list.is_empty() {
            out += &format!("\nNo changes in {} binary channels.\n", binary.len());
            print!("{out}");
            return Ok(());
        }

        // The trigger goes into the timeline when it lies in the window.
        let trigger_row = |prev: Option<f64>| {
            vec![
                signed(0.0),
                prev.map_or_else(String::new, |p| format!("+{}", time(-p, span))),
                absolute(0.0),
                "trigger".to_owned(),
                String::new(),
            ]
        };
        let mut rows = Vec::new();
        let mut prev: Option<f64> = None;
        let mut trigger_shown = !(t0..=t1).contains(&0.0);
        for e in &list {
            if !trigger_shown && e.time >= 0.0 {
                rows.push(trigger_row(prev));
                prev = Some(0.0);
                trigger_shown = true;
            }
            rows.push(vec![
                signed(e.time),
                prev.map_or_else(String::new, |p| format!("+{}", time(e.time - p, span))),
                absolute(e.time),
                format!("D{} {}", e.index, e.id),
                format!("{} → {}", u8::from(e.from), u8::from(e.to)),
            ]);
            prev = Some(e.time);
        }
        if !trigger_shown {
            rows.push(trigger_row(prev));
        }
        out += &format!("\n{}", table(&["time", "Δ", "absolute", "channel", "change"], &rows));
        out += &format!("\n{} changes in {} binary channels\n", list.len(), binary.len());
        print!("{out}");
        Ok(())
    }
}
