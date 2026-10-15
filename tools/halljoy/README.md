# HallJoy import tools

These scripts build Depthpad's keyboard data for the protocols ported from
[HallJoy](https://github.com/PashOK7/HallJoy) (AGPL-3.0) out of a HallJoy checkout. Their output is
committed; run them again only to pick up changes in HallJoy.

```
set HALLJOY_DIR=C:\path\to\HallJoy          (default: a HallJoy checkout next to this repository)
set ANALOGSENSE_JS=C:\path\to\AnalogSense.js  (only for gen_keychron_extra.py)
python hj_layouts.py      HallJoy's layouts, in key units  -> hj_layouts.json (not committed)
python gen_rongyuan.py    RongYuan and Attack Shark boards -> protocols/halljoy/rongyuan_tables.rs
python gen_hex80.py       ATK / MADLIONS Hex80 models      -> protocols/halljoy/hex80_tables.rs
python gen_ipi.py         IPI / Royal Kludge models        -> protocols/halljoy/ipi_tables.rs
python gen_jingtai.py     IROK / WLMOUSE / JingTai boards  -> protocols/halljoy/jingtai_tables.rs
python gen_keychron_extra.py  Keychron K4/K6/Q2/Q4, Lemokey ISO -> protocols/analogsense_tables.rs
python gen.py             families and variants (families.py) -> data/devices, data/variants
python gen_docs.py        docs/KEYBOARDS.md and the README's keyboard summary
```

Then run `cargo test -p depthpad-core`: it checks every model and variant (keys drawn once, no overlaps,
the index matching the files).
