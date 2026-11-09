# Analysis methods

`phasors`, `trend`, `harmonics` and `impedance` share one way of looking at
the signals. This page describes it, so the numbers can be compared with what
a protection relay or another analysis tool shows.

## The window

Every value is computed over a **window of whole cycles of the fundamental
frequency** (`--cycles`, default 1) that **ends** at a sample: at `--at` for
the commands that look at one moment, at every step for `trend` and
`impedance --trend`. This is how a protection relay sees the signal at that
moment: only what happened before.

The fundamental frequency is the line frequency in the `.cfg` (`--frequency`
overrides it). The number of samples per window comes from the sample spacing
at the window's end, for example 40 samples for one cycle at 2 kHz and 50 Hz.
A window must have at least 4 samples and must not start before the recording;
the error message names the option to change.

## Sliding RMS

The RMS view of the viewer (`r`) uses the same window: one cycle ending at each
sample, so it shows what a relay sees and a step shows up over the following cycle.

- **With DC** it is the true RMS of the window's samples: √(mean(v²)). At the
  nominal frequency this is the same value as `trend --rms`.
- **Without DC** (`d`) the mean of the window is removed: √(mean(v²) − mean(v)²).
  This also takes out most of a decaying offset in a fault current.
- There is no value until a whole window fits, around a missing sample, or with
  fewer than 4 samples per window.
- **The cycle is fitted to the signal**, so the value doesn't ripple off the nominal
  frequency: it is the last period between rising zero crossings of one reference
  channel for all channels. The reference is the strongest voltage with a signal,
  otherwise the strongest current. Where no period can be measured, or it is above
  1.5 times the line frequency, the window is one cycle of the line frequency. The
  footer names the reference when the view is switched on.

## Phasors

A phasor is computed with a DFT at the fundamental over the window's samples,
using the **sample times** rather than the sample numbers, so the angle is
right even when a cycle is not a whole number of samples. A signal
x(t) = A·cos(ωt + φ) gives the phasor A·e^jφ.

- **Magnitudes** are shown as **RMS** values (peak / √2).
- **Angles** are in degrees relative to a reference channel (`--reference`,
  default the first selected channel), in (−180°, 180°].
- A **DC offset** is rejected over whole cycles, so a decaying fault current
  offset changes the fundamental only slightly.
- Values are in **primary** units, or secondary with `--secondary`; each
  channel is converted with the ratios in the `.cfg`.
- A missing sample in the window leaves that channel without a value; the other
  channels are still computed.

## Symmetrical components

Three channels with phases A, B and C (also written L1/L2/L3 or R/S/T) of the
**same circuit and unit** form a group. For each group:

- zero sequence: (A + B + C) / 3
- positive sequence: (A + a·B + a²·C) / 3
- negative sequence: (A + a²·B + a·C) / 3

with a = 1∠120°. The **unbalance** is negative / positive sequence in %. In the
`trend` columns they are written `I1`, `I2`, `I0` and `I2/1` (the label is the
common start of the channel ids: IA, IB, IC → `I`).

A phase to ground fault shows as **equal negative and zero sequence current**;
a phase to phase fault as negative sequence without zero sequence.

## Frequency

`trend` estimates the system frequency from how far the reference channel's
phasor **turns** between two steps Δt apart: a signal at f₀ + Δf turns by
2π·Δf·Δt, so f = f₀ + Δφ / (2π·Δt). This is valid while |Δf|·Δt < ½, that is
within ±25 Hz at the default step of one cycle. The first row and rows where
the reference is zero (breaker open) have no frequency.

The `frequency` command measures one channel over `--cycles` cycles N ending
at each row, with one of two methods:

- **Zero crossings** (default): the time of each rising zero crossing is
  interpolated between the two samples around it; the frequency is N periods
  over the time between the first and the last of N + 1 crossings. A crossing
  counts only after the signal was below −5 % of the channel's largest value
  (hysteresis), so noise around zero adds none. A constant DC offset doesn't
  change the periods. The measurement stands for the middle of its periods,
  N/2 cycles back; it is gone when the last crossing is more than 1.5 nominal
  periods old (the signal stopped) or a period is longer than that (a pause,
  such as the dead time of a reclose).
- **Phasor**: the turn of the fundamental phasor between the window of N
  cycles ending at the row and the one before it, as above. Off the nominal
  frequency, the DFT of a single phase has an image that makes the angle
  ripple; the measurement is repeated twice with windows of whole cycles of
  the frequency found (frequency tracking), which removes it. It stands for
  the middle of both windows, N cycles back, and needs the phasor above 5 % of
  the channel's largest value.

The **ROCOF** (df/dt) compares each measurement with the one of the N cycles
before it, over the time between their middles. A phase jump (fault
inception, switching) shows as a short frequency excursion with a large ROCOF;
over one cycle the ROCOF is also noisy from the sampling, so frequency events
are best measured over 5 or more cycles.

## Harmonics

`harmonics` runs the DFT at each multiple of the fundamental over the same
window; over whole cycles this is exact for harmonics. Orders above half the
sample rate cannot be represented by the samples, so the highest order is
limited (to the 19th at 40 samples per cycle).

- DC (the mean over the window) and each harmonic are given in % of the
  fundamental.
- **THD** is the RMS of the harmonics 2 … max over the fundamental, labelled
  with the orders it covers, because it depends on `--max-order`.
- **Angles** are relative to the channel's fundamental, shifted by the order:
  φₕ − h·φ₁. This describes the waveform's shape and does not change with
  where the window starts.

A decaying DC offset is not periodic; besides the DC value it shows as small
harmonics that decrease with the order.

## Fault loop impedance

`impedance` computes the six loops a distance relay measures from a voltage
and a current A/B/C group (found by unit, or given with `--voltage` and
`--current`), scaled to volts and amperes:

- phase to ground (A-N, B-N, C-N): Z = V / (I + k0·3I0), with
  3I0 = I_A + I_B + I_C
- phase to phase (A-B, B-C, C-A): Z = (V₁ − V₂) / (I₁ − I₂)

k0 = (Z0 − Z1) / (3·Z1) is the line's residual compensation factor
(`--k0 0.8@-5` for magnitude and angle). Without it (the default 0), the phase
to ground loops are not compensated, which the output says. Results are R, X,
|Z| and φ in Ω, primary or secondary.

The loop voltage and current are shown next to each loop, because an impedance
only means something when the loop carries fault current. `--trend` gives R and
X per loop over time, the trajectory to compare with the relay's zones.

## Power

`power` uses the same voltage and current groups as `impedance`. Per phase the
complex power is S = V·I* with RMS phasors: P = Re S is the active power in W,
Q = Im S the reactive power in var, |S| the apparent power in VA. The total is
the sum of the three phases, and the power factor is P / |S|, marked `lag`
when Q > 0 (inductive) and `lead` when Q < 0.

P > 0 means power flows in the direction the currents are measured, usually
from the busbar into the line; reverse flow gives P < 0 and a negative power
factor. The voltages must be phase to neutral.

## Fault detection

`fault`, `dc-offset` and `locate` find the fault inception from the superimposed current
Δi(n) = i(n) − i(n − N), N samples per cycle: in steady state it is close to
0, a fault makes it jump. Inception is the first of 3 samples in a row where
|Δi| of a phase is above 20 % of the pre-fault peak (at least 2 % of the
largest current). The faulted phases come from the change of the phasors
against the cycle before inception, ΔI = I(fault) − I(pre-fault), so load
current doesn't count: a phase is faulted when its |ΔI| is at least half the
largest, ground is involved when 3·|ΔI0| is at least 20 % of it.

## DC offset

`dc-offset` fits each phase current from inception over a few cycles as

i(t) = a·sin(ωt) + b·cos(ωt) + D·e^(−t/τ)

the current of an R-L circuit switched onto a fault. For a fixed τ the model
is linear in a, b and D, solved by least squares; τ is searched from 1 ms to
1 s on a logarithmic grid of 64 values and refined by golden section. τ = L/R
of the fault loop, so X/R = ω·τ. D depends on the point on the wave at
inception: largest at a voltage zero, none at a voltage peak.

The τ of a tiny DC term is noise, so τ is shown only for a DC part of at least
5 % of the AC peak. The model has one exponential: a saturating CT, an arc
whose resistance changes, or two sources with different X/R make it fit
worse, which the fit error (RMS of the residual in % of the AC peak) shows.

## Fault location

`locate` measures the faulted loop (A-N, B-C, …) with voltage V and current I,
ground loops compensated as I + k0·3I0 with k0 = (Z0 − Z1) / (3·Z1) from the
line data. The loop sees V = d·z1·I + Rf·If, with the distance d, the line's
z1 per km, the fault resistance Rf and the fault current If.

- **Reactance method:** d = Im(V / I) / Im(z1). Exact for a bolted fault.
  With fault resistance, Rf·If / I adds reactance whenever If and I differ in
  phase: load flow before the fault and infeed from the other line end make
  the fault look nearer (exporting end) or farther (importing end).
- **Takagi:** d = Im(V·ΔI*) / Im(z1·I·ΔI*) with the superimposed current
  ΔI, which flows only because of the fault and is nearly in phase with If.
  The Rf term then is (nearly) real and drops out. It's exact when the source
  and line impedances have the same angle; in strongly non-homogeneous
  systems an error remains.

The measurement window is one cycle in the middle of the fault, starting at
least one cycle after inception, so the DC offset of the fault current has
decayed. Both methods need the line data to be right: a result beyond the
line end often means wrong z1 or z0 rather than a fault on the next line.

## Comparing two recordings

`compare` aligns B's time axis to A's with an offset d, t_A = t_B + d: from
the absolute trigger times, from the triggers, or from the fault inceptions.
Each recording's phasors come from a DFT over its own samples, so the sample
rates may differ. The DFT refers the angle to the recording's own time
axis, so B's phasor is turned into A's frame: P_B · e^(−jω·d). An alignment
error of 1 ms is 18° at 50 Hz (21.6° at 60 Hz): angles between unsynchronized
recorders mean little, magnitudes still compare.

For two line ends, with both currents measured into the line, the
differential current I_diff = |I_A + I_B| is close to 0 for load and for
faults outside the line (the current passes through), and close to the
fault current for a fault on it. The restraint current
I_restraint = (|I_A| + |I_B|) / 2 is what a differential relay compares it with.
The line's charging current and CT errors show as a small I_diff.
`compare --trend` computes it cycle by cycle, with B's windows ending at the
same instants as A's, so the rise at inception and the fall at each
breaker opening show.

## Noise

Recordings store integer samples, so computed values carry tiny residues:
1e−8 of the signal where the true value is zero. Components and impedance
parts below 1e−6 of the largest value in their group are shown as 0, and
results are rounded to 12 significant digits, so this residue does not look
like a measurement.
