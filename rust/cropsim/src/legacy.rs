//! Readers for the original CropSim text input files (BLOCK.DAT,
//! TILLAGE.DAT, NESOILS.DAT, *.SIM, *.WEA, INITIAL.DAT), ported from the
//! Python package's `cropsim.legacy` so both read a file the same way.
//!
//! Numbers are parsed as f64 and then rounded to f32, as Python does, so
//! both sides hand the kernel identical values.
//!
//! The readers follow Fortran list-directed input: each READ starts on a
//! new line, takes values across as many lines as it needs, and ignores the
//! rest of its last line.

use std::collections::BTreeMap;
use std::path::Path;

use crate::model::{Block, Sim, Soil, State, Tillage, Weather};
use crate::Error;

/// Drainage values the original looked up by soil code (DCOFSOIL, DDAYSOIL).
const SOIL_CODES: [i32; 28] = [
    411, 412, 421, 422, 431, 432, 442, 512, 521, 522, 532, 542, 612, 621, 622, 631, 632, 642, 721, 722, 731,
    732, 821, 822, 831, 832, 921, 922,
];
const DRAIN_DAYS: [f32; 28] = [
    2.0, 2.0, 2.0, 2.0, 4.0, 4.0, 6.0, 2.0, 2.0, 2.0, 4.0, 6.0, 3.0, 3.0, 3.0, 4.0, 4.0, 6.0, 3.0, 3.0, 4.0,
    4.0, 4.0, 4.0, 6.0, 6.0, 4.0, 4.0,
];
const DRAIN_COEFF: f32 = 0.50;

fn read_text(path: &Path) -> Result<String, Error> {
    let bytes = std::fs::read(path).map_err(|e| Error::Io { path: path.display().to_string(), source: e })?;
    // The old files are Latin-1 at worst; every number is ASCII.
    Ok(bytes.iter().map(|&b| b as char).collect())
}

/// Fortran list-directed READ over a text file.
pub struct ListReader {
    lines: Vec<String>,
    pos: usize,
    path: String,
}

impl ListReader {
    pub fn new(text: &str, path: &str) -> ListReader {
        ListReader { lines: text.lines().map(str::to_owned).collect(), pos: 0, path: path.into() }
    }

    fn err(&self, message: impl Into<String>) -> Error {
        Error::Parse { path: self.path.clone(), message: message.into() }
    }

    pub fn skip(&mut self, n: usize) {
        self.pos += n;
    }

    pub fn read(&mut self, n: usize) -> Result<Vec<String>, Error> {
        let mut values: Vec<String> = Vec::with_capacity(n);
        while values.len() < n {
            if self.pos >= self.lines.len() {
                return Err(self.err("unexpected end of file"));
            }
            let line = &self.lines[self.pos];
            self.pos += 1;
            for tok in line.split(|c: char| c == ',' || c.is_whitespace()) {
                if tok.is_empty() {
                    continue;
                }
                if tok.starts_with('/') {
                    break;
                }
                if let Some((count, val)) = tok.split_once('*') {
                    let count: usize = count.parse().map_err(|_| Error::Parse {
                        path: self.path.clone(),
                        message: format!("bad repeat {tok:?}"),
                    })?;
                    values.extend(std::iter::repeat_n(val.to_string(), count));
                } else {
                    values.push(tok.trim_matches(|c| c == '\'' || c == '"').to_string());
                }
            }
        }
        values.truncate(n);
        Ok(values)
    }

    fn num(&self, v: &str) -> Result<f64, Error> {
        v.parse::<f64>().map_err(|_| self.err(format!("not a number: {v:?}")))
    }

    pub fn floats(&mut self, n: usize) -> Result<Vec<f32>, Error> {
        let v = self.read(n)?;
        v.iter().map(|s| self.num(s).map(|x| x as f32)).collect()
    }

    pub fn ints(&mut self, n: usize) -> Result<Vec<i32>, Error> {
        let v = self.read(n)?;
        v.iter().map(|s| self.num(s).map(|x| x as i32)).collect()
    }

    fn f(&self, s: &str) -> Result<f32, Error> {
        self.num(s).map(|x| x as f32)
    }

    fn i(&self, s: &str) -> Result<i32, Error> {
        self.num(s).map(|x| x as i32)
    }

    pub fn at_end(&self) -> bool {
        self.lines[self.pos.min(self.lines.len())..].iter().all(|l| l.trim().is_empty())
    }
}

fn arr<const N: usize>(v: Vec<f32>) -> [f32; N] {
    v.try_into().expect("reader returned the requested count")
}

fn iarr<const N: usize>(v: Vec<i32>) -> [i32; N] {
    v.try_into().expect("reader returned the requested count")
}

/// Fortran Fw.d input: a field without a decimal point has d implied decimals.
fn fixed_float(field: &str, decimals: i32) -> f32 {
    let f: String = field.chars().filter(|c| *c != ' ').collect();
    if f.is_empty() {
        return 0.0;
    }
    if f.contains('.') || f.to_ascii_lowercase().contains('e') {
        return f.parse::<f64>().unwrap_or(0.0) as f32;
    }
    (f.parse::<i64>().unwrap_or(0) as f64 / 10f64.powi(decimals)) as f32
}

fn fixed_int(field: &str) -> i32 {
    let f: String = field.chars().filter(|c| *c != ' ').collect();
    f.parse().unwrap_or(0)
}

/// Characters `a..b` of a line, padded with blanks.
fn cols(line: &str, a: usize, b: usize) -> String {
    line.chars().chain(std::iter::repeat(' ')).skip(a).take(b - a).collect()
}

pub fn read_block(path: impl AsRef<Path>) -> Result<Block, Error> {
    let path = path.as_ref();
    let mut r = ListReader::new(&read_text(path)?, &path.display().to_string());
    let v = r.read(3)?;
    let mut b = Block { etrfact: r.f(&v[0])?, nfday: r.i(&v[1])?, ytrend: r.i(&v[2])?, ..Block::default() };
    b.depth = arr(r.floats(10)?);
    b.ydens = arr(r.floats(22)?);
    b.resrat = arr(r.floats(22)?);
    b.spgrav = arr(r.floats(22)?);
    b.fragil = iarr(r.ints(22)?);
    b.restyp = iarr(r.ints(22)?);
    for crop in b.cc.iter_mut().take(7) {
        crop[0] = arr(r.floats(10)?);
        crop[1] = arr(r.floats(10)?);
    }
    for stage in b.cc[9].iter_mut() {
        *stage = arr(r.floats(10)?); // alfalfa
    }
    b.kcl = arr(r.floats(22)?);
    b.kcu = arr(r.floats(22)?);
    b.tbase = arr(r.floats(22)?);
    b.tceil = arr(r.floats(22)?);
    b.snow = arr(r.floats(7)?);
    b.cnfallow = arr(r.floats(4)?);
    for crop in b.cn.iter_mut() {
        *crop = arr(r.floats(4)?);
    }
    Ok(b)
}

/// TILLAGE.DAT: two header lines, then FORMAT(I4,2(2X,F7.0),5X,A20).
pub fn read_tillage(path: impl AsRef<Path>) -> Result<Vec<Tillage>, Error> {
    let text = read_text(path.as_ref())?;
    Ok(text
        .lines()
        .skip(2)
        .filter(|l| !l.trim().is_empty())
        .map(|line| Tillage {
            code: fixed_int(&cols(line, 0, 4)),
            factor1: fixed_float(&cols(line, 6, 13), 0),
            factor2: fixed_float(&cols(line, 15, 22), 0),
            name: cols(line, 27, 47).trim().to_string(),
        })
        .collect())
}

/// NESOILS.DAT: one header line, then code, BULKD, ORGM, FIELDC(10),
/// PWP(10), AIRDRY(2). The first row for a code wins.
pub fn read_soils(path: impl AsRef<Path>) -> Result<BTreeMap<i32, Soil>, Error> {
    let path = path.as_ref();
    let mut r = ListReader::new(&read_text(path)?, &path.display().to_string());
    r.skip(1);
    let mut soils = BTreeMap::new();
    while !r.at_end() {
        let v = r.floats(25)?;
        let code = v[0] as i32;
        soils.entry(code).or_insert_with(|| Soil {
            soil: code,
            bulkd: v[1],
            orgm: v[2],
            fieldc: v[3..13].try_into().unwrap(),
            pwp: v[13..23].try_into().unwrap(),
            airdry: v[23..25].try_into().unwrap(),
            drncoe: DRAIN_COEFF,
            drnday: SOIL_CODES.iter().position(|&c| c == code).map_or(2.0, |k| DRAIN_DAYS[k]),
        });
    }
    Ok(soils)
}

/// Read a .SIM file (original main program lines 828-910). Values the
/// file's crop type does not set stay at their defaults (zero); pass the
/// previously read Sim as `previous` to [`read_sim_after`] to reproduce the
/// original program, which kept them from the last SIM file.
pub fn read_sim(path: impl AsRef<Path>) -> Result<Sim, Error> {
    read_sim_after(path, None)
}

pub fn read_sim_after(path: impl AsRef<Path>, previous: Option<&Sim>) -> Result<Sim, Error> {
    let path = path.as_ref();
    let mut s = previous.cloned().unwrap_or_default();
    let mut r = ListReader::new(&read_text(path)?, &path.display().to_string());
    let v = r.ints(2)?;
    (s.crop, s.layers) = (v[0], v[1]);
    let v = r.ints(6)?;
    (s.implt, s.idplt, s.imbg, s.idbg, s.imend, s.idend) = (v[0], v[1], v[2], v[3], v[4], v[5]);
    if s.crop < 10 {
        let v = r.floats(6)?;
        (s.gddveg, s.gddflo, s.gddripe, s.gyform, s.gddefc, s.gddmat) = (v[0], v[1], v[2], v[3], v[4], v[5]);
    }
    if s.crop == 10 || s.crop == 11 {
        let v = r.ints(6)?;
        s.ncut = v[0];
        s.jdycut = v[1..6].try_into().unwrap();
    }
    if s.crop >= 12 {
        let v = r.floats(2)?;
        (s.gddefc, s.gddmat) = (v[0], v[1]);
    }
    let v = r.floats(5)?;
    (s.rzmin, s.rzmax, s.tbreak, s.gddroot, s.rzmgmt) = (v[0], v[1], v[2], v[3], v[4]);
    let v = r.read(3)?;
    (s.iyield, s.ymax, s.ycoeff) = (r.i(&v[0])?, r.f(&v[1])?, r.f(&v[2])?);
    let v = r.read(4)?;
    (s.iterrc, s.chand, s.chanw, s.tintrv) = (r.i(&v[0])?, r.f(&v[1])?, r.f(&v[2])?, r.f(&v[3])?);
    let v = r.ints(2)?;
    let ntills = v[0].max(0) as usize;
    s.itflag = v[1];
    s.tillage_ops = (0..ntills).map(|_| r.ints(3).map(|t| (t[0], t[1], t[2]))).collect::<Result<_, _>>()?;
    let v = r.read(6)?;
    s.irrtyp = r.i(&v[0])?;
    for k in 0..5 {
        s.pad[k] = r.f(&v[k + 1])?;
    }
    let v = r.floats(6)?;
    s.rainal = v[0..5].try_into().unwrap();
    s.syscap = v[5];
    s.iper = arr(r.floats(5)?);
    s.apmin = arr(r.floats(5)?);
    s.apmax = arr(r.floats(5)?);
    s.smalli = arr(r.floats(5)?);
    let v = r.read(4)?;
    (s.gstart, s.gstop, s.irrsch, s.jfirst) = (r.f(&v[0])?, r.f(&v[1])?, r.i(&v[2])?, r.i(&v[3])?);
    if s.irrsch == 3 {
        read_rotation(&mut r, &mut s)?;
    }
    s.eapp = arr(r.floats(5)?);
    s.ereuse = arr(r.floats(5)?);
    s.prunof = arr(r.floats(5)?);
    if s.irrtyp == 2 {
        s.fdfile = r.read(1)?.remove(0);
        let v = r.floats(3)?;
        s.alpha1 = v[0];
        s.sim_airdry = [v[1], v[2]];
        s.sim_fieldc = arr(r.floats(10)?);
        s.sim_pwp = arr(r.floats(10)?);
        s.sim_bulkd = r.floats(1)?[0];
    }
    Ok(s)
}

/// How many numbers a line starts with (up to a '/' or a word that is not
/// a number, such as a comment).
fn leading_numbers(line: &str) -> usize {
    let mut n = 0;
    for tok in line.split(|c: char| c == ',' || c.is_whitespace()) {
        if tok.is_empty() {
            continue;
        }
        if tok.starts_with('/') {
            break;
        }
        if let Some((count, val)) = tok.split_once('*') {
            match (count.parse::<usize>(), val.parse::<f64>()) {
                (Ok(c), Ok(_)) => n += c,
                _ => break,
            }
        } else if tok.parse::<f64>().is_ok() {
            n += 1;
        } else {
            break;
        }
    }
    n
}

/// The IRRSCH 3 rotation line, in either layout (see the Python
/// `cropsim.legacy._read_rotation`).
fn read_rotation(r: &mut ListReader, s: &mut Sim) -> Result<(), Error> {
    while r.pos < r.lines.len() && r.lines[r.pos].trim().is_empty() {
        r.pos += 1;
    }
    let n = r.lines.get(r.pos).map_or(0, |l| leading_numbers(l));
    let forage = (10..=13).contains(&s.crop);
    let full = |r: &mut ListReader, s: &mut Sim| -> Result<(), Error> {
        let v = r.read(4)?;
        (s.jstopi, s.jgoi, s.jdfreq, s.ddepth) = (r.i(&v[0])?, r.i(&v[1])?, r.i(&v[2])?, r.f(&v[3])?);
        Ok(())
    };
    let short = |r: &mut ListReader, s: &mut Sim| -> Result<(), Error> {
        let v = r.read(2)?;
        (s.jdfreq, s.ddepth) = (r.i(&v[0])?, r.f(&v[1])?);
        Ok(())
    };
    if n >= 4 {
        full(r, s)
    } else if n == 2 || forage {
        short(r, s)
    } else {
        full(r, s)
    }
}

/// Read an FDFILE of known irrigations: KIRR, then day, gross depth, fraction.
pub fn read_irrigation_events(path: impl AsRef<Path>) -> Result<Vec<(i32, f32, f32)>, Error> {
    let path = path.as_ref();
    let mut r = ListReader::new(&read_text(path)?, &path.display().to_string());
    let n = r.ints(1)?[0].max(0) as usize;
    (0..n)
        .map(|_| {
            let v = r.read(3)?;
            Ok((r.i(&v[0])?, r.f(&v[1])?, r.f(&v[2])?))
        })
        .collect()
}

/// Read a .WEA file (original subroutine READWEAT). `year` defaults to the
/// four digits before `.WEA` in the file name.
pub fn read_weather(path: impl AsRef<Path>, year: Option<i32>) -> Result<Weather, Error> {
    let path = path.as_ref();
    let text = read_text(path)?;
    let lines: Vec<&str> = text.lines().collect();
    let perr = |m: &str| Error::Parse { path: path.display().to_string(), message: m.into() };
    let head = lines.get(1).ok_or_else(|| perr("missing header line"))?;
    // FORMAT(2I3,I6,7X,F7.2,8X,F8.2,12X,F8.0)
    let imstr = fixed_int(&cols(head, 0, 3));
    let idstr = fixed_int(&cols(head, 3, 6));
    let lat = fixed_float(&cols(head, 19, 26), 2);
    let lon = fixed_float(&cols(head, 34, 42), 2);
    let elev = fixed_float(&cols(head, 54, 62), 0);
    let body = lines.get(3..).unwrap_or(&[]).join("\n");
    let mut r = ListReader::new(&body, &path.display().to_string());
    let mut w = Weather {
        year: 0,
        start_month: imstr,
        start_day: idstr,
        lat,
        lon,
        elev,
        tmax: Vec::with_capacity(366),
        tmin: Vec::with_capacity(366),
        precip: Vec::with_capacity(366),
        epan: Vec::with_capacity(366),
        etr: Vec::with_capacity(366),
    };
    while !r.at_end() {
        let v = r.floats(6)?;
        w.tmax.push(v[1]);
        w.tmin.push(v[2]);
        w.precip.push(v[3]);
        w.epan.push(v[4]);
        w.etr.push(v[5]);
    }
    w.year = match year {
        Some(y) => y,
        None => year_from_name(path).unwrap_or(0),
    };
    Ok(w)
}

fn year_from_name(path: &Path) -> Option<i32> {
    let name = path.file_name()?.to_str()?.to_ascii_uppercase();
    let stem = name.strip_suffix(".WEA")?;
    let digits = stem.get(stem.len().checked_sub(4)?..)?;
    digits.parse().ok()
}

/// Read INITIAL.DAT (carryover state).
pub fn read_initial(path: impl AsRef<Path>) -> Result<State, Error> {
    let path = path.as_ref();
    let mut r = ListReader::new(&read_text(path)?, &path.display().to_string());
    let theta = arr(r.floats(10)?);
    let v = r.read(3)?;
    let v2 = r.floats(12)?;
    Ok(State {
        theta,
        oldcrp: r.i(&v[0])?,
        oldres: r.f(&v[1])?,
        livecrop: r.i(&v[2])?,
        snotmp: v2[0],
        snoh2o: v2[1],
        soilt: v2[2..12].try_into().unwrap(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_reader_repeats_and_slash() {
        let mut r = ListReader::new("1, 2*3.5 / ignored\n4\n", "t");
        assert_eq!(r.floats(4).unwrap(), vec![1.0, 3.5, 3.5, 4.0]);
    }

    #[test]
    fn fixed_fields() {
        assert_eq!(fixed_float("  4125", 2), 41.25);
        assert_eq!(fixed_float(" 41.25", 2), 41.25);
        assert_eq!(fixed_int("  7 "), 7);
    }

    #[test]
    fn year_from_file_name() {
        assert_eq!(year_from_name(Path::new("x/SCTB2014.WEA")), Some(2014));
        assert_eq!(year_from_name(Path::new("x/SCTB.txt")), None);
    }
}
