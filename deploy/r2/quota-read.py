#!/usr/bin/env python3
"""Linux/ext4-Userquoten nur lesen (Q_GETQUOTA; niemals Q_SETQUOTA).

ABI gemäß /usr/include/linux/quota.h. Keine Abhängigkeiten, kein Netzwerk.
Ausgabe enthält nur öffentliche Grenzwerte, keine Dateinamen oder Inhalte.
"""
import argparse
import ctypes
import errno
import sys


class Dqblk(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in (
        "bhard", "bsoft", "space", "ihard", "isoft", "inodes", "btime", "itime"
    )] + [("valid", ctypes.c_uint32)]


def query(device: str, uid: int) -> Dqblk:
    libc = ctypes.CDLL(None, use_errno=True)
    call = libc.quotactl
    call.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_void_p]
    call.restype = ctypes.c_int
    data = Dqblk()
    # QCMD(Q_GETQUOTA, USRQUOTA), als signed 32-bit übergeben.
    command = ctypes.c_int(0x800007 << 8).value
    if call(command, device.encode(), uid, ctypes.byref(data)) != 0:
        code = ctypes.get_errno()
        raise OSError(code, errno.errorcode.get(code, "Quota-Abfrage fehlgeschlagen"))
    return data


def limits_match(data: Dqblk, blocks: int, inodes: int) -> bool:
    return (data.bhard == data.bsoft == blocks
            and data.ihard == data.isoft == inodes
            and data.space <= blocks * 1024 and data.inodes <= inodes)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("device")
    parser.add_argument("uid", type=int)
    parser.add_argument("blocks", type=int, help="Hardlimit in KiB; Softlimit identisch")
    parser.add_argument("inodes", type=int)
    args = parser.parse_args()
    if args.uid < 1000 or args.blocks <= 0 or args.inodes <= 0:
        parser.error("System-UID / unbegrenztes Budget nicht erlaubt")
    try:
        data = query(args.device, args.uid)
    except OSError as error:
        print(f"Quota nicht nachweisbar: {error.strerror}", file=sys.stderr)
        return 1
    if not limits_match(data, args.blocks, args.inodes):
        print("Quota oder aktuelle Belegung weicht vom vereinbarten Budget ab", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
