#!/usr/bin/env python3
"""Génère les rapports DS4 bruts utilisés par tests/ds4.rs.

Rapports synthétiques construits octet par octet d'après les structures du driver Linux
(drivers/hid/hid-playstation.c : dualshock4_input_report_usb/_bt, touch_point,
calibration) ; les CRC32 sont calculés avec zlib, indépendamment du code Rust testé.

Usage : python3 generate.py  (réécrit les fichiers .hex à côté du script)
Format : octets hexadécimaux séparés par des espaces, commentaires après « # ».
"""

import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))


def touch_point(active, ident, x, y):
    contact = (ident & 0x7F) | (0 if active else 0x80)
    return bytes([contact, x & 0xFF, ((x >> 8) & 0x0F) | ((y & 0x0F) << 4), (y >> 4) & 0xFF])


def common(sticks, b0, b1, b2, l2, r2, timestamp, gyro, accel, status0, status1):
    data = bytes(sticks) + bytes([b0, b1, b2, l2, r2])
    data += struct.pack("<H", timestamp) + bytes([0x1A])  # sensor_temperature
    data += struct.pack("<3h", *gyro) + struct.pack("<3h", *accel)
    data += bytes(5) + bytes([status0, status1]) + bytes(1)
    assert len(data) == 32
    return data


def touch_report(ts, p0, p1):
    return bytes([ts]) + touch_point(*p0) + touch_point(*p1)


def usb_report(common_data, touch_reports):
    data = bytes([0x01]) + common_data + bytes([len(touch_reports)])
    padded = list(touch_reports) + [bytes(9)] * (3 - len(touch_reports))
    data += b"".join(padded) + bytes(3)
    assert len(data) == 64
    return data


def bt_report(common_data, touch_reports):
    data = bytes([0x11, 0xC0, 0x00]) + common_data + bytes([len(touch_reports)])
    padded = list(touch_reports) + [bytes(9)] * (4 - len(touch_reports))
    data += b"".join(padded) + bytes(2)
    assert len(data) == 74
    crc = zlib.crc32(bytes([0xA1]) + data)
    return data + struct.pack("<I", crc)


def calibration(report_id, layout):
    bias = (-3, 2, 1)
    pitch, yaw, roll = (8800, -8780), (8790, -8810), (8805, -8795)
    if layout == "usb":
        gyro = (pitch[0], pitch[1], yaw[0], yaw[1], roll[0], roll[1])
    else:
        gyro = (pitch[0], yaw[0], roll[0], pitch[1], yaw[1], roll[1])
    speed = (540, 540)
    accel = (8200, -8180, 8250, -8150, 8220, -8190)
    data = bytes([report_id]) + struct.pack("<3h6h2h6h", *bias, *gyro, *speed, *accel)
    assert len(data) == 35
    if report_id == 0x02:
        return data + bytes(37 - 35)
    data += bytes(41 - 4 - 35)
    crc = zlib.crc32(bytes([0xA3]) + data)
    return data + struct.pack("<I", crc)


def write(name, data, comment):
    path = os.path.join(HERE, name)
    with open(path, "w", encoding="utf-8") as f:
        f.write(f"# {comment}\n")
        for i in range(0, len(data), 16):
            f.write(" ".join(f"{b:02x}" for b in data[i:i + 16]) + "\n")


# Croix : droite (2). Boutons : ✕ (bit 5) + △ (bit 7). L1, L2, Options, R3. PS + clic pavé.
# L2 à 0x40, R2 à 0xFF. Câble branché, batterie 7 → 75 % en charge.
USB_COMMON = common(
    sticks=(0x80, 0x80, 0x00, 0xFF), b0=0x02 | 0x20 | 0x80, b1=0x01 | 0x04 | 0x20 | 0x80, b2=0x01 | 0x02 | 0xA8,
    l2=0x40, r2=0xFF, timestamp=0x1234, gyro=(100, -200, 300), accel=(-50, 8192, 400), status0=0x10 | 7,
    status1=0x00,
)
write(
    "ds4_usb_full.hex",
    usb_report(USB_COMMON, [touch_report(0x22, (True, 5, 1919, 941), (False, 6, 0, 0))]),
    "USB 0x01 : croix droite, ✕ △ L1 L2 Options R3 PS clic pavé ; 1 doigt (id 5, 1919×941) ; 75 % en charge",
)

# Bluetooth : rien d'appuyé, croix relâchée (8), sans câble, batterie 3 → 35 %. Deux doigts.
# Deux touch_reports : le second (le plus récent) est celui qui doit être retenu.
BT_COMMON = common(
    sticks=(0x00, 0xFF, 0x80, 0x80), b0=0x08, b1=0x00, b2=0x00, l2=0x00, r2=0x00, timestamp=0xFFF0,
    gyro=(-1, 0, 32767), accel=(0, 0, -8192), status0=0x03, status1=0x00,
)
write(
    "ds4_bt_full.hex",
    bt_report(BT_COMMON, [
        touch_report(0x10, (True, 1, 10, 20), (False, 2, 0, 0)),
        touch_report(0x11, (True, 1, 100, 200), (True, 2, 1500, 700)),
    ]),
    "BT 0x11 : stick gauche en bas à gauche, 2 doigts (100,200) et (1500,700), 35 % sur batterie, CRC valide",
)

write(
    "ds4_bt_full_no_touch.hex",
    bt_report(BT_COMMON, [touch_report(0x12, (False, 1, 100, 200), (False, 2, 1500, 700))]),
    "BT 0x11 : aucun doigt sur le pavé",
)

# Rapport réduit : 0x01 + 9 premiers octets de common. Croix haut-gauche (7), □ (bit 4), R1.
write(
    "ds4_bt_reduced.hex",
    bytes([0x01, 0x80, 0x80, 0xC0, 0x40, 0x07 | 0x10, 0x02, 0x00, 0x00, 0x80]),
    "BT 0x01 réduit : stick droit (0xC0, 0x40), croix haut-gauche, □, R1, R2 à moitié",
)

write("ds4_calibration_usb.hex", calibration(0x02, "usb"), "Feature 0x02 (USB) : calibration IMU")
write("ds4_calibration_bt.hex", calibration(0x05, "bt"), "Feature 0x05 (BT) : calibration IMU, CRC valide")

# Rapport de sortie BT attendu : vibration forte 0x80 / faible 0x40, barre lumineuse (0x11, 0x22, 0x33).
out = bytearray(78)
out[0], out[1], out[3] = 0x11, 0xC4, 0x03
out[6], out[7] = 0x40, 0x80
out[8:11] = bytes([0x11, 0x22, 0x33])
out[74:78] = struct.pack("<I", zlib.crc32(bytes([0xA2]) + bytes(out[:74])))
write("ds4_output_bt.hex", bytes(out), "Sortie BT 0x11 attendue : fort 0x80, faible 0x40, RVB 11 22 33, CRC32 (graine 0xA2)")
