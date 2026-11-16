# Credits

The renderer is written from DEC's VT330/VT340 Programmer Reference Manual,
Volume 2: Graphics Programming (HTML edition at vt100.net), the
specification every command here follows. No code in this repository was
copied from anywhere. The works below are credited because data of theirs
is here: fonts generated from them, a character set and test references.

- **DEC VWSVT0 terminal font** (`fonts/vwsvt0_19.bdf`, Digital Equipment
  Corporation freeware, c. 1990, as converted to BDF in hackerb9's
  vt340test). It supplies the shapes of the built-in characters, which the
  manual does not give. `fonts/vwsvt0.regisfont` is generated from it.

- **VT340 character generator ROM 23-236E6** (Digital Equipment
  Corporation; dumped by Malcolm Macleod, avitech.com.au, and kept at
  bitsavers.org). `fonts/vt340.regisfont` is generated from it: the
  VT340's own ReGIS shapes for 0x20-0x7E and its text shapes for
  0xA0-0xFF. The ROM itself is not kept here.

- **Greek loadable character set** (`fonts/loadable/greek.regis`, from
  hackerb9's vt340test, CC0). A ready-made set for the load command.

- **hackerb9's vt340test** (CC0, https://github.com/hackerb9/vt340test).
  Its screen captures from a real VT340 are the references the renderer is
  measured against (`crates/regis/tests/data/vt340-*.pbm`), and its ReGIS
  test scripts supply the commands for them. It showed that a VT340 fills
  polygons made of position moves and dots, draws size-1 text from twenty
  distinct rows, and ignores a stray character inside brackets.
