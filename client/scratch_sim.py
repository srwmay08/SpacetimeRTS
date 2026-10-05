import math

def planck_blackbody_rgb(t):
    wavelengths = [6.80e-7, 5.50e-7, 4.40e-7]
    h = 6.62607015e-34
    c = 2.99792458e8
    k = 1.380649e-23
    res = []
    for l in wavelengths:
        exp_val = (h * c) / (l * k * t)
        if exp_val > 700.0: exp_term = math.exp(700.0)
        else: exp_term = math.exp(exp_val)
        res.append((2.0 * h * c * c) / (l**5 * (exp_term - 1.0)))
    m = max(res)
    return [r / m for r in res]

def optical_air_mass(deg):
    if deg >= 0.0:
        rad = math.radians(deg)
        denom = math.sin(rad) + 0.50572 * (deg + 6.07995)**(-1.6364)
        return max(1.0, min(38.2, 1.0 / max(1e-4, denom)))
    else:
        return min(80.0, 38.2 + (-deg) * 7.0)

def optical_air_mass_ozone(deg):
    if deg >= 0.0:
        r_ratio = 6371.0 / (6371.0 + 25.0)
        rad = math.radians(deg)
        cos_e = math.cos(rad)
        denom = math.sqrt(max(1e-4, 1.0 - r_ratio * r_ratio * cos_e * cos_e))
        return max(1.0, min(11.35, 1.0 / denom))
    else:
        return max(11.35, min(38.0, 11.35 + (-deg) * 2.5))

def evaluate_transmittance(deg):
    m_r = optical_air_mass(deg)
    m_o = optical_air_mass_ozone(deg)
    tau = [
        0.0058 * 8.4 * m_r + 0.0084 * 1.2 * m_r + 0.00065 * 25.0 * m_o,
        0.0135 * 8.4 * m_r + 0.0084 * 1.2 * m_r + 0.00240 * 25.0 * m_o,
        0.0331 * 8.4 * m_r + 0.0084 * 1.2 * m_r + 0.000085 * 25.0 * m_o,
    ]
    return [math.exp(-t) for t in tau]

def tonemap(rad):
    luma = rad[0] * 0.2126 + rad[1] * 0.7152 + rad[2] * 0.0722
    t_day = max(0.0, min(1.0, luma / 0.12))
    s_day = t_day * t_day * (3.0 - 2.0 * t_day)
    t_night = 1.0 - max(0.0, min(1.0, luma / 0.035))
    s_night = t_night * t_night * (3.0 - 2.0 * t_night)
    exp_day = 1.0 / (luma + 0.15)
    exp_twilight = 1.0 / (luma + 0.04)
    exp_night = max(15.0, min(32.0, 1.0 / (luma + 0.02)))
    exposure = exp_day * s_day + exp_twilight * (1.0 - s_day) * (1.0 - s_night) + exp_night * s_night
    exp_c = [r * exposure for r in rad]
    return [max(0.0, min(1.0, c / (1.0 + c * 0.8))) for c in exp_c]

def sim_horizon(hour):
    angle = (hour - 12.0) * (math.pi / 12.0)
    elev_a_deg = 68.0 * math.cos(angle)
    elev_b_deg = 62.0 * math.cos(angle - 0.25)

    trans_a = evaluate_transmittance(elev_a_deg)
    trans_b = evaluate_transmittance(elev_b_deg)
    bb_a = planck_blackbody_rgb(5800.0)
    bb_b = planck_blackbody_rgb(3900.0)

    filt_a = [bb_a[i] * trans_a[i] for i in range(3)]
    filt_b = [bb_b[i] * trans_b[i] for i in range(3)]

    t_a = max(0.0, min(1.0, (elev_a_deg + 8.0) / 12.0))
    cutoff_a = t_a * t_a * (3.0 - 2.0 * t_a)
    t_b = max(0.0, min(1.0, (elev_b_deg + 8.0) / 12.0))
    cutoff_b = t_b * t_b * (3.0 - 2.0 * t_b)

    lux_a = 50000.0 * (sum(trans_a)/3.0) * cutoff_a
    lux_b = 35000.0 * (sum(trans_b)/3.0) * cutoff_b
    total_lux = lux_a + lux_b

    # Sunset Hermite weight
    s_a = max(0.0, min(1.0, (20.0 - elev_a_deg) / 20.0))
    w_a = s_a * s_a * (3.0 - 2.0 * s_a)
    s_b = max(0.0, min(1.0, (20.0 - elev_b_deg) / 20.0))
    w_b = s_b * s_b * (3.0 - 2.0 * s_b)

    mie_a = [filt_a[i] * (lux_a * 0.000045) * w_a for i in range(3)]
    mie_b = [filt_b[i] * (lux_b * 0.000045) * w_b for i in range(3)]

    bell_a = math.exp(-((elev_a_deg + 3.5) / 5.5)**2)
    twi_a = [
        0.85 * min(1.2, math.exp(-elev_a_deg * 0.10)) * bell_a * (lux_a / 50000.0),
        0.005 * bell_a * (lux_a / 50000.0),
        0.35 * max(0.0, min(1.0, (elev_a_deg + 10.0)/10.0)) * bell_a * (lux_a / 50000.0)
    ]
    bell_b = math.exp(-((elev_b_deg + 3.5) / 5.5)**2)
    twi_b = [
        0.65 * min(1.0, math.exp(-elev_b_deg * 0.10)) * bell_b * (lux_b / 35000.0),
        0.003 * bell_b * (lux_b / 35000.0),
        0.25 * max(0.0, min(1.0, (elev_b_deg + 10.0)/10.0)) * bell_b * (lux_b / 35000.0)
    ]

    starlight = [0.0058 * 24.0 * 0.08, 0.0135 * 0.08 * 0.08, 0.0331 * 18.0 * 0.08]

    # Daytime Rayleigh horizon skylight
    day_f = max(0.0, min(1.0, total_lux / 25000.0))
    smooth_day = day_f * day_f * (3.0 - 2.0 * day_f)
    ray_coef = [0.0058, 0.0135, 0.0331]
    horizon_rayleigh = [
        ray_coef[0] * 2.2 * (total_lux * 0.000038) * smooth_day,
        ray_coef[1] * 1.55 * (total_lux * 0.000038) * smooth_day,
        ray_coef[2] * 1.0 * (total_lux * 0.000038) * smooth_day,
    ]

    horizon_rad = [
        mie_a[i] + mie_b[i] + twi_a[i] + twi_b[i] + starlight[i]*1.5 + horizon_rayleigh[i]
        for i in range(3)
    ]
    col = tonemap(horizon_rad)
    return elev_a_deg, elev_b_deg, col

print(f"{'Hour':<6} {'Elev A':<8} {'Elev B':<8} {'R':<6} {'G':<6} {'B':<6} {'Atmospheric Phase & Hue'}")
print("-" * 75)
for h in [6.0, 7.0, 8.0, 10.0, 12.0, 14.0, 16.0, 17.0, 17.5, 18.0, 18.5, 19.0, 20.0, 22.0, 24.0]:
    ea, eb, col = sim_horizon(h)
    desc = ""
    if ea > 15.0 and eb > 15.0:
        desc = "Daytime Sky Blue (Clean, Airy Rayleigh Blue)"
    elif ea > 0.0 or eb > 0.0:
        if col[0] > col[2] * 1.05:
            desc = "Golden Amber / Blaze Orange Sunset Gradient"
        else:
            desc = "Late Afternoon Golden-Blue Transition"
    elif ea > -10.0 or eb > -10.0:
        desc = "Belt of Venus Twilight Arch (Fuchsia / Violet)"
    else:
        desc = "Electric Navy Deep Night (Starfield & Aurora)"
    print(f"{h:<6.1f} {ea:<8.1f} {eb:<8.1f} {col[0]:<6.3f} {col[1]:<6.3f} {col[2]:<6.3f} {desc}")
