"""Reproducible original Light-Field & Glass physical-modeled UI cues (G major)."""
from pathlib import Path
import json
import math
import struct
import wave

RATE = 44100
ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "Assets/Sounds"

# G 大调（含 Maj7 / Maj9 水晶扩展音）纯正音律频率
G4 = 392.0000
A4 = 440.0000
B4 = 493.8833
D5 = 587.3295
Fs5 = 739.9888
G5 = 783.9909
A5 = 880.0000
B5 = 987.7666
D6 = 1174.6591

# 物理模态模型：(频率比, 振幅权重, 阻尼倍数)
MODELS = {
    "frosted": [
        (1.000, 1.00, 1.0),
        (1.998, 0.14, 2.4),
        (2.860, 0.04, 4.2),
    ],
    "glass": [
        (1.000, 1.00, 1.0),
        (2.003, 0.24, 1.9),
        (2.756, 0.11, 3.2),
        (4.120, 0.045, 5.0),
    ],
    "crystal": [
        (1.000, 1.00, 1.0),
        (2.001, 0.30, 1.6),
        (2.996, 0.14, 2.5),
        (4.420, 0.065, 3.8),
        (5.890, 0.025, 5.5),
    ],
    "liquid": [
        (1.000, 1.00, 1.0),
        (2.012, 0.12, 2.6),
    ],
    "wood": [
        (1.000, 1.00, 1.0),
        (1.580, 0.28, 2.8),
        (2.440, 0.12, 4.8),
    ],
}

# 每款音色的精确规格：(时长ms, 模型, 光学颤光cents, 敲击感, 空间反射强度, 音符列表)
# 音符元组：(freq, start_s, decay_s, level, sweep_semitones, attack_s)
SPECS = {
    # 柔点：温润磨砂玻璃轻触（G5 柔和轻叩 + 低八度暖光托底）
    "notification-soft.wav": (
        180,
        "frosted",
        2.8,
        0.04,
        0.16,
        [
            (G5, 0.000, 0.052, 0.90, 0.0, 0.0035),
            (G4, 0.000, 0.048, 0.32, 0.0, 0.0040),
        ],
    ),
    # 澄光：切工水晶双音折射闪光（D5 -> G5 晶莹折射）
    "notification-glass.wav": (
        220,
        "crystal",
        3.6,
        0.06,
        0.22,
        [
            (D5, 0.000, 0.058, 0.78, 0.0, 0.0022),
            (G5, 0.028, 0.064, 0.85, 0.0, 0.0022),
            (D6, 0.028, 0.036, 0.18, 0.0, 0.0025),
        ],
    ),
    # 轻铃：硼硅玻璃风铃温暖三音和声（G4 - B4 - D5 - G5）
    "todo-bell.wav": (
        300,
        "glass",
        3.2,
        0.05,
        0.24,
        [
            (G4, 0.000, 0.088, 0.72, 0.0, 0.0030),
            (B4, 0.022, 0.080, 0.56, 0.0, 0.0030),
            (D5, 0.044, 0.086, 0.68, 0.0, 0.0028),
            (G5, 0.062, 0.068, 0.28, 0.0, 0.0030),
        ],
    ),
    # 相遇：晶格磁吸确认上行双音（G4 -> D5 -> G5）
    "device-connect.wav": (
        240,
        "glass",
        3.4,
        0.05,
        0.20,
        [
            (G4, 0.000, 0.052, 0.78, 0.0, 0.0025),
            (D5, 0.068, 0.064, 0.94, 0.0, 0.0024),
            (G5, 0.068, 0.045, 0.26, 0.0, 0.0028),
        ],
    ),
    # 气泡：液态玻璃水滴弹跳滑音
    "bubble.wav": (
        160,
        "liquid",
        2.0,
        0.03,
        0.14,
        [
            (G4 * 1.06, 0.000, 0.044, 0.95, 5.5, 0.0024),
            (G5 * 1.06, 0.014, 0.026, 0.22, 3.0, 0.0028),
        ],
    ),
    # 花开：极光流光 Maj7 水晶三音绽放（G4 -> B4 -> D5 -> F#5）
    "bloom.wav": (
        340,
        "glass",
        4.2,
        0.04,
        0.26,
        [
            (G4, 0.000, 0.095, 0.72, 0.0, 0.0035),
            (B4, 0.046, 0.086, 0.76, 0.0, 0.0032),
            (D5, 0.094, 0.084, 0.86, 0.0, 0.0030),
            (Fs5, 0.138, 0.070, 0.36, 0.0, 0.0035),
        ],
    ),
    # 涟漪：静水光波递减晶莹回响（G5 -> D5 -> B5）
    "ripple.wav": (
        320,
        "crystal",
        3.8,
        0.04,
        0.25,
        [
            (G5, 0.000, 0.062, 0.88, 0.0, 0.0028),
            (D5, 0.075, 0.056, 0.56, 0.0, 0.0032),
            (G5, 0.148, 0.048, 0.32, 0.0, 0.0036),
            (B5, 0.205, 0.036, 0.16, 0.0, 0.0040),
        ],
    ),
    # 微风：柔光气声缓入 + 柔和双音玻璃共鸣
    "breeze.wav": (
        280,
        "frosted",
        3.5,
        0.015,
        0.22,
        [
            (D5 / 2.0, 0.000, 0.096, 0.82, 0.0, 0.0120),
            (G4, 0.016, 0.092, 0.72, 0.0, 0.0140),
            (B4, 0.036, 0.078, 0.34, 0.0, 0.0150),
        ],
    ),
    # 木语：温润木质叩击对比音色
    "wood.wav": (
        180,
        "wood",
        1.2,
        0.14,
        0.10,
        [
            (G4 / 2.0, 0.000, 0.034, 0.96, -1.1, 0.0018),
            (G4, 0.000, 0.020, 0.34, 0.0, 0.0018),
        ],
    ),
    # 上扬：三阶晶透折光轻跃（G4 -> A4 -> D5 -> G5）
    "rise.wav": (
        320,
        "glass",
        3.5,
        0.045,
        0.22,
        [
            (G4, 0.000, 0.056, 0.76, 0.0, 0.0026),
            (A4, 0.064, 0.056, 0.82, 0.0, 0.0026),
            (D5, 0.128, 0.064, 0.92, 0.0, 0.0024),
            (G5, 0.184, 0.050, 0.34, 0.0, 0.0028),
        ],
    ),
    # 落点：两音温润玻璃阻尼收束（D5 -> G4）
    "drop.wav": (
        240,
        "frosted",
        2.6,
        0.04,
        0.18,
        [
            (D5, 0.000, 0.050, 0.72, 0.0, 0.0025),
            (G4, 0.074, 0.066, 0.96, -0.35, 0.0028),
        ],
    ),
    # 和鸣：光场水晶 Maj9 完整和弦与空气尾韵（G4-B4-D5-F#5-A5）
    "finish.wav": (
        400,
        "crystal",
        4.4,
        0.05,
        0.28,
        [
            (G4, 0.000, 0.125, 0.76, 0.0, 0.0032),
            (B4, 0.034, 0.112, 0.64, 0.0, 0.0030),
            (D5, 0.068, 0.106, 0.68, 0.0, 0.0028),
            (Fs5, 0.104, 0.092, 0.44, 0.0, 0.0032),
            (A5, 0.140, 0.074, 0.28, 0.0, 0.0036),
        ],
    ),
}


def deterministic_click(u: float, freq: float) -> float:
    """确定性伪噪声敲击瞬态，模拟毛毡锤轻触玻璃/木块瞬间的质感。"""
    if u < 0.0 or u > 0.0045:
        return 0.0
    env = math.exp(-u / 0.0011) * (1.0 - math.exp(-u / 0.0003))
    # 多个非有理频比合成短促带限瞬态
    grain = (
        math.sin(2.0 * math.pi * (freq * 3.17) * u)
        + 0.6 * math.sin(2.0 * math.pi * (freq * 5.83) * u + 1.1)
        + 0.35 * math.sin(2.0 * math.pi * 4180.0 * u + 2.3)
    )
    return env * grain * 0.45


def synth(ms: int, model_name: str, shimmer_cents: float, click_amt: float, room_amt: float, notes):
    n = round(ms * RATE / 1000)
    total_sec = ms / 1000.0
    modes = MODELS[model_name]
    detune_hi = 2.0 ** (shimmer_cents / 1200.0)
    detune_lo = 2.0 ** (-shimmer_cents / 1200.0)

    dry = [0.0] * n
    for idx in range(n):
        t = idx / RATE
        v = 0.0
        for freq, start, decay, level, sweep, attack_tau in notes:
            u = t - start
            if u < 0.0:
                continue
            attack = (1.0 - math.exp(-u / attack_tau)) ** 2
            env = attack * math.exp(-u / decay)
            sweep_term = (2.0 ** (sweep / 12.0) - 1.0) * 0.022 * (1.0 - math.exp(-u / 0.022))
            base_phase = 2.0 * math.pi * freq * (u + sweep_term)

            tone = 0.0
            for ratio, m_weight, m_damp in modes:
                m_env = math.exp(-u * (m_damp - 1.0) / max(decay, 0.02))
                p = base_phase * ratio
                if ratio <= 2.1:
                    # 光学微失谐双声部制造玻璃流光感
                    part = 0.55 * math.sin(p * detune_hi) + 0.45 * math.sin(p * detune_lo + 0.35)
                else:
                    part = math.sin(p + 0.2 * ratio)
                tone += m_weight * m_env * part

            if click_amt > 0.0:
                tone += click_amt * deterministic_click(u, freq)

            v += level * env * tone
        dry[idx] = v

    # 极短微型空间早期反射，并叠加简易一阶低通让尾韵温润通透
    taps = [
        (round(0.009 * RATE), 0.24),
        (round(0.017 * RATE), 0.16),
        (round(0.027 * RATE), 0.10),
        (round(0.039 * RATE), 0.055),
    ]
    wet = [0.0] * n
    lp = 0.0
    for i in range(n):
        r_sum = 0.0
        for d_samp, gain in taps:
            src_i = i - d_samp
            if src_i >= 0:
                r_sum += dry[src_i] * gain
        lp = 0.58 * lp + 0.42 * r_sum
        # 轻微 tanh 润色，消除数字尖锐感
        combined = math.tanh((dry[i] + room_amt * lp) * 1.15)
        wet[i] = combined

    # 严格起止安全窗：前 1.8ms 二次渐变淡入（保证前 1ms < peak/2），末尾 38ms 三次平滑淡出（保证末 1ms < 200 且尾采样归零）
    fade_in = max(1, round(0.0018 * RATE))
    fade_out_sec = min(0.042, total_sec * 0.24)
    for i in range(n):
        t = i / RATE
        if i < fade_in:
            k = i / fade_in
            wet[i] *= k * k
        rem = total_sec - t
        if rem < fade_out_sec:
            r = max(0.0, min(1.0, rem / fade_out_sec))
            wet[i] *= r * r * (3.0 - 2.0 * r)

    peak_raw = max(abs(x) for x in wet) or 1.0
    # 精确归一化至 -13.0 dBFS（0.22387 * 32767 ≈ 7335）
    target_peak = 0.2238 * 32767.0
    samples = [round(x / peak_raw * target_peak) for x in wet]
    samples[0] = 0
    samples[-1] = 0
    return samples


def write_wav(path: Path, samples):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(struct.pack("<" + "h" * len(samples), *samples))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    report = []
    for name, (ms, model_name, shimmer, click_amt, room_amt, notes) in SPECS.items():
        samples = synth(ms, model_name, shimmer, click_amt, room_amt, notes)
        write_wav(OUT / name, samples)
        peak = max(abs(v) for v in samples)
        rms = math.sqrt(sum(v * v for v in samples) / len(samples))
        report.append(
            {
                "file": name,
                "model": model_name,
                "milliseconds": round(len(samples) * 1000.0 / RATE, 3),
                "peakDbfs": round(20.0 * math.log10(peak / 32768.0), 2),
                "rmsDbfs": round(20.0 * math.log10(max(rms, 1e-9) / 32768.0), 2),
                "start": samples[0],
                "end": samples[-1],
            }
        )
    print(json.dumps({"success": True, "cues": report}, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
