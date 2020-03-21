use std::collections::HashMap;
use std::fmt;
use std::io::Write;

#[derive(Debug, Copy, Clone)]
struct SmallBuf {
    bytes: [u8; 8],
    len: u8,
}

impl SmallBuf {
    fn new() -> Self {
        Self {
            bytes: [0; 8],
            len: 0,
        }
    }

    fn append(&self, other: Self) -> Self {
        if self.len + other.len > 8 {
            panic!("SmallBuf overflow")
        }
        let mut out = *self;
        for &byte in &other.bytes[..(other.len as usize)] {
            out.push(byte);
        }
        out
    }

    fn push(&mut self, byte: u8) {
        if self.len == 8 {
            panic!("SmallBuf overflow");
        }
        self.bytes[self.len as usize] = byte;
        self.len += 1;
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes[..(self.len as usize)]
    }
}

impl std::ops::Add for SmallBuf {
    type Output = SmallBuf;

    fn add(self, rhs: Self) -> Self {
        self.append(rhs)
    }
}

impl std::ops::AddAssign for SmallBuf {
    fn add_assign(&mut self, rhs: Self) {
        *self = self.append(rhs)
    }
}

#[derive(Debug, Copy, Clone)]
enum DiskId {
    D0,
    D1,
}

impl DiskId {
    fn assemble(&self) -> u8 {
        match self {
            DiskId::D0 => 0,
            DiskId::D1 => 1,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Register {
    A,
    B,
    C,
    D,
    I,
    J,
    P,
    S,
}

impl Register {
    fn assemble(&self) -> u8 {
        match self {
            Register::A => 0,
            Register::B => 1,
            Register::C => 2,
            Register::D => 3,
            Register::I => 4,
            Register::J => 5,
            Register::P => 6,
            Register::S => 7,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Format {
    Binary,
    Decimal,
    Hexadecimal,
}

#[derive(Debug, Copy, Clone)]
struct Number {
    value: u16,
    format: Format,
}

#[derive(Debug, Copy, Clone)]
struct Name<'a> {
    name: &'a str,
    fragment: Fragment<'a>,
}

#[derive(Debug, Copy, Clone)]
enum Constant<'a> {
    Name(Name<'a>),
    Number(Number),
}

impl<'a> Constant<'a> {
    fn assemble(&self, labels: &HashMap<&str, u16>) -> Result<(u8, SmallBuf), Error<'a>> {
        match *self {
            Constant::Name(Name { name, fragment }) => {
                match labels.get(name).copied() {
                    None => Err(Error {
                        fragment,
                        message: "undefined label",
                    }),
                    Some(x) => {
                        let mut buf = SmallBuf::new();
                        let [low, high] = x.to_le_bytes();
                        buf.push(low);
                        buf.push(high);
                        Ok((0xe, buf))
                    }
                }
            }
            Constant::Number(Number { value, .. }) => {
                match value {
                    0 => Ok((0x8, SmallBuf::new())),
                    1 => Ok((0x9, SmallBuf::new())),
                    2 => Ok((0xa, SmallBuf::new())),
                    3 => Ok((0xb, SmallBuf::new())),
                    4 => Ok((0xc, SmallBuf::new())),
                    5 ..= 255 => {
                        let mut buf = SmallBuf::new();
                        buf.push(value as u8);
                        Ok((0xd, buf))
                    }
                    256 ..= 0xfffe => {
                        let mut buf = SmallBuf::new();
                        let [low, high] = value.to_le_bytes();
                        buf.push(low);
                        buf.push(high);
                        Ok((0xe, buf))
                    }
                    0xffff => Ok((0xf, SmallBuf::new())),
                }
            }
        }
    }

    fn value(&self, labels: &HashMap<&str, u16>) -> Result<u16, Error<'a>> {
        match *self {
            Constant::Name(Name { name, fragment }) => {
                match labels.get(name).copied() {
                    None => Err(Error {
                        fragment,
                        message: "undefined label",
                    }),
                    Some(x) => Ok(x),
                }
            }
            Constant::Number(Number { value, .. }) => Ok(value),
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Value<'a> {
    Register(Register),
    Constant(Constant<'a>),
}

impl<'a> Value<'a> {
    fn assemble(&self, labels: &HashMap<&str, u16>) -> Result<(u8, SmallBuf), Error<'a>> {
        match *self {
            Value::Register(r) => Ok((r.assemble(), SmallBuf::new())),
            Value::Constant(c) => c.assemble(labels),
        }
    }
}

#[derive(Debug, Copy, Clone)]
struct Address<'a> {
    value: Value<'a>,
    offset: Option<Constant<'a>>,
}

#[derive(Debug, Clone)]
enum Instruction<'a> {
    Nop,
    Ret,
    Wait,
    Poll,
    Not(Register),
    Neg(Register),
    Pop(Register),
    Push(Value<'a>),
    Jmp(Value<'a>),
    Call(Value<'a>),
    Mov(Register, Value<'a>),
    Add(Register, Value<'a>),
    Sub(Register, Value<'a>),
    Xor(Register, Value<'a>),
    And(Register, Value<'a>),
    Or(Register, Value<'a>),
    Shl(Register, Value<'a>),
    Shr(Register, Value<'a>),
    Cmp(Register, Value<'a>),
    Load(Register, Address<'a>),
    Loadw(Register, Address<'a>),
    Store(Value<'a>, Address<'a>),
    Storew(Value<'a>, Address<'a>),
    Jez(Register, Constant<'a>),
    Jnz(Register, Constant<'a>),
    Jl(Register, Constant<'a>),
    Jg(Register, Constant<'a>),
    Jle(Register, Constant<'a>),
    Jge(Register, Constant<'a>),
    Read(DiskId, Constant<'a>, Constant<'a>),
    Write(DiskId, Constant<'a>, Constant<'a>),
    Bytes(Vec<Number>),
}

impl<'a> Instruction<'a> {
    fn assemble(&self, into: &mut Vec<u8>, labels: &HashMap<&str, u16>) -> Result<(), Error<'a>> {
        let mut output = SmallBuf::new();
        match self {
            Instruction::Nop => output.push(0x00),
            Instruction::Ret => output.push(0x01),
            Instruction::Wait => output.push(0x02),
            Instruction::Poll => output.push(0x03),
            Instruction::Not(r) => output.push(0x10 + r.assemble()),
            Instruction::Neg(r) => output.push(0x20 + r.assemble()),
            Instruction::Pop(r) => output.push(0x30 + r.assemble()),
            Instruction::Push(v) => {
                let (x, b) = v.assemble(labels)?;
                output.push(0x40 + x);
                output += b;
            }
            Instruction::Jmp(v) => {
                let (x, b) = v.assemble(labels)?;
                output.push(0x50 + x);
                output += b;
            }
            Instruction::Call(v) => {
                let (x, b) = v.assemble(labels)?;
                output.push(0x60 + x);
                output += b;
            }
            Instruction::Mov(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x80);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Add(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x81);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Sub(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x82);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Xor(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x83);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::And(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x84);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Or(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x85);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Shl(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x86);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Shr(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x87);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Cmp(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0x88);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Load(a, Address { value, offset: None }) => {
                let a = a.assemble();
                let (value, buf) = value.assemble(labels)?;
                output.push(0x90);
                output.push((a << 4) + value);
                output += buf;
            }
            Instruction::Load(a, Address { value, offset: Some(c) }) => {
                let a = a.assemble();
                let (value, buf) = value.assemble(labels)?;
                output.push(0x92);
                output.push((a << 4) + value);
                let [low, high] = c.value(labels)?.to_le_bytes();
                output.push(low);
                output.push(high);
                output += buf;
            }
            Instruction::Loadw(a, Address { value, offset: None }) => {
                let a = a.assemble();
                let (value, buf) = value.assemble(labels)?;
                output.push(0x94);
                output.push((a << 4) + value);
                output += buf;
            }
            Instruction::Loadw(a, Address { value, offset: Some(c) }) => {
                let a = a.assemble();
                let (value, buf) = value.assemble(labels)?;
                output.push(0x96);
                output.push((a << 4) + value);
                let [low, high] = c.value(labels)?.to_le_bytes();
                output.push(low);
                output.push(high);
                output += buf;
            }
            Instruction::Store(a, Address { value, offset: None }) => {
                let (a, abuf) = a.assemble(labels)?;
                let (value, buf) = value.assemble(labels)?;
                output.push(0x98);
                output.push((a << 4) + value);
                output += abuf;
                output += buf;
            }
            Instruction::Store(a, Address { value, offset: Some(c) }) => {
                let (a, abuf) = a.assemble(labels)?;
                let (value, buf) = value.assemble(labels)?;
                output.push(0x9a);
                output.push((a << 4) + value);
                let [low, high] = c.value(labels)?.to_le_bytes();
                output.push(low);
                output.push(high);
                output += abuf;
                output += buf;
            }
            Instruction::Storew(a, Address { value, offset: None }) => {
                let (a, abuf) = a.assemble(labels)?;
                let (value, buf) = value.assemble(labels)?;
                output.push(0x9c);
                output.push((a << 4) + value);
                output += abuf;
                output += buf;
            }
            Instruction::Storew(a, Address { value, offset: Some(c) }) => {
                let (a, abuf) = a.assemble(labels)?;
                let (value, buf) = value.assemble(labels)?;
                output.push(0x9e);
                output.push((a << 4) + value);
                let [low, high] = c.value(labels)?.to_le_bytes();
                output.push(low);
                output.push(high);
                output += abuf;
                output += buf;
            }
            Instruction::Jez(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0xa0);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Jnz(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0xa1);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Jl(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0xa2);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Jg(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0xa3);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Jle(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0xa4);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Jge(a, b) => {
                let a = a.assemble();
                let (b, buf) = b.assemble(labels)?;
                output.push(0xa5);
                output.push((a << 4) + b);
                output += buf;
            }
            Instruction::Read(d, a, b) => {
                let d = d.assemble();
                let (a, ab) = a.assemble(labels)?;
                let (b, bb) = b.assemble(labels)?;
                output.push(0xf0 + d);
                output.push((a << 4) + b);
                output += ab;
                output += bb;
            }
            Instruction::Write(d, a, b) => {
                let d = d.assemble();
                let (a, ab) = a.assemble(labels)?;
                let (b, bb) = b.assemble(labels)?;
                output.push(0xf8 + d);
                output.push((a << 4) + b);
                output += ab;
                output += bb;
            }
            Instruction::Bytes(bytes) => {
                for b in bytes {
                    into.push(b.value as u8);
                }
            }
        }

        into.extend(output.as_slice().iter().copied());
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Line<'a> {
    label: Option<&'a str>,
    instruction: Option<Instruction<'a>>,
}

#[derive(Debug, Copy, Clone)]
struct Fragment<'a> {
    line: &'a str,
    line_number: usize,
    start: usize,
    end: usize,
}

impl<'a> Fragment<'a> {
    fn new(line: &'a str, line_number: usize) -> Self {
        Fragment {
            line,
            line_number,
            start: 0,
            end: line.len(),
        }
    }

    fn prefix(&self, len: usize) -> Self {
        Fragment {
            line: self.line,
            line_number: self.line_number,
            start: self.start,
            end: std::cmp::min(self.start + len, self.end),
        }
    }

    fn suffix(&self, len: usize) -> Self {
        Fragment {
            line: self.line,
            line_number: self.line_number,
            start: std::cmp::max(self.start, self.end.saturating_sub(len)),
            end: self.end,
        }
    }

    fn trim(&self) -> Self {
        let x = self.suffix(self.as_str().trim_start().len());
        x.prefix(x.as_str().trim_end().len())
    }

    fn as_str(&self) -> &'a str {
        &self.line[self.start..self.end]
    }

    fn split_on(&self, c: char) -> (Self, Option<(Self, Self)>) {
        if let Some(idx) = self.as_str().find(c) {
            let a = self.prefix(idx);
            let b = self.suffix(self.len() - idx).prefix(c.len_utf8());
            let c = self.suffix(self.len() - idx - c.len_utf8());
            (a, Some((b, c)))
        } else {
            (*self, None)
        }
    }

    fn len(&self) -> usize {
        self.end - self.start
    }
}

#[derive(Debug, Copy, Clone)]
struct Error<'a> {
    fragment: Fragment<'a>,
    message: &'static str,
}

impl<'a> Error<'a> {
    fn or(&self, other: Self) -> Self {
        if self.fragment.len() == 0 {
            other
        } else {
            *self
        }
    }
}

impl<'a> fmt::Display for Error<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error: {}", self.message)?;
        writeln!(f, "{: >3} | {}", self.fragment.line_number, self.fragment.line)?;
        write!(f, "    | ")?;
        for _ in 0..self.fragment.start {
            write!(f, " ")?;
        }
        for _ in self.fragment.start..self.fragment.end {
            write!(f, "^")?;
        }
        writeln!(f)
    }
}

trait ParseArg<'a>: Sized {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>>;
}

impl<'a> ParseArg<'a> for DiskId {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        let fragment = fragment.trim();
        match fragment.as_str() {
            "0" => Ok(DiskId::D0),
            "1" => Ok(DiskId::D1),
            _ => Err(Error {
                fragment,
                message: "invalid disk",
            }),
        }
    }
}

impl<'a> ParseArg<'a> for Register {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        let fragment = fragment.trim();
        match fragment.as_str() {
            "a" | "A" => Ok(Register::A),
            "b" | "B" => Ok(Register::B),
            "c" | "C" => Ok(Register::C),
            "d" | "D" => Ok(Register::D),
            "i" | "I" => Ok(Register::I),
            "j" | "J" => Ok(Register::J),
            "p" | "P" => Ok(Register::P),
            "s" | "S" => Ok(Register::S),
            _ => Err(Error {
                fragment,
                message: "invalid register",
            }),
        }
    }
}

impl<'a> ParseArg<'a> for Number {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        let fragment = fragment.trim();
        if fragment.as_str().starts_with("0x") || fragment.as_str().starts_with("0X") {
            let rest = &fragment.as_str()[2..];
            if let Ok(value) = u16::from_str_radix(rest, 16) {
                return Ok(Number {
                    value,
                    format: Format::Hexadecimal,
                });
            }
        } else if fragment.as_str().starts_with("0b") || fragment.as_str().starts_with("0B") {
            let rest = &fragment.as_str()[2..];
            if let Ok(value) = u16::from_str_radix(rest, 2) {
                return Ok(Number {
                    value,
                    format: Format::Binary,
                });
            }
        }
        if let Ok(value) = u16::from_str_radix(fragment.as_str(), 10) {
            return Ok(Number {
                value,
                format: Format::Decimal,
            });
        } else {
            Err(Error {
                fragment,
                message: "invalid number",
            })
        }
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some('a' ..= 'z') |
        Some('A' ..= 'Z') |
        Some('_') => {}
        _ => return false,
    }
    for ch in chars {
        match ch {
            'a' ..= 'z' |
            'A' ..= 'Z' |
            '0' ..= '9' |
            '_' => {}
            _ => return false,
        }
    }
    true
}

impl<'a> ParseArg<'a> for Name<'a> {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        let fragment = fragment.trim();
        if is_identifier(fragment.as_str()) {
            Ok(Name {
                name: fragment.as_str(),
                fragment,
            })
        } else {
            Err(Error {
                fragment,
                message: "invalid name",
            })
        }
    }
}

impl<'a> ParseArg<'a> for Constant<'a> {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        let fragment = fragment.trim();
        if let Ok(_) = Register::parse(fragment) {
            Err(Error {
                fragment,
                message: "invalid constant",
            })
        } else if let Ok(num) = Number::parse(fragment) {
            Ok(Constant::Number(num))
        } else if let Ok(name) = Name::parse(fragment) {
            Ok(Constant::Name(name))
        } else {
            Err(Error {
                fragment,
                message: "invalid value",
            })
        }
    }
}

impl<'a> ParseArg<'a> for Value<'a> {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        if let Ok(reg) = Register::parse(fragment) {
            Ok(Value::Register(reg))
        } else if let Ok(constant) = Constant::parse(fragment) {
            Ok(Value::Constant(constant))
        } else {
            Err(Error {
                fragment: fragment.trim(),
                message: "invalid value",
            })
        }
    }
}

impl<'a> ParseArg<'a> for Address<'a> {
    fn parse(fragment: Fragment<'a>) -> Result<Self, Error<'a>> {
        let fragment = fragment.trim();
        let (value, offset) = fragment.split_on('+');
        let value = Value::parse(value)?;
        let offset = if let Some((plus, offset)) = offset {
            Some(Constant::parse(offset).map_err(|e| e.or(Error {
                fragment: plus,
                message: "missing offset",
            }))?)
        } else {
            None
        };
        Ok(Address {
            value,
            offset,
        })
    }
}

fn parse0<'a>(
    fragment: Fragment<'a>,
    _opcode: Fragment<'a>,
    instruction: Instruction<'a>,
) -> Result<Instruction<'a>, Error<'a>> {
    let fragment = fragment.trim();
    if fragment.len() == 0 {
        Ok(instruction)
    } else {
        Err(Error {
            fragment,
            message: "expected no arguments",
        })
    }
}

fn parse1<'a, A: ParseArg<'a>, F: FnOnce(A) -> Instruction<'a>>(
    fragment: Fragment<'a>,
    opcode: Fragment<'a>,
    instruction: F,
) -> Result<Instruction<'a>, Error<'a>> {
    let fragment = fragment.trim();
    if fragment.len() == 0 {
        Err(Error {
            fragment: opcode,
            message: "expected one argument",
        })
    } else if let (_, Some(_)) = fragment.split_on(',') {
        Err(Error {
            fragment,
            message: "expected one argument",
        })
    } else {
        let arg = A::parse(fragment)?;
        Ok(instruction(arg))
    }
}

fn parse2<'a, A: ParseArg<'a>, B: ParseArg<'a>, F: FnOnce(A, B) -> Instruction<'a>>(
    fragment: Fragment<'a>,
    opcode: Fragment<'a>,
    instruction: F,
) -> Result<Instruction<'a>, Error<'a>> {
    let fragment = fragment.trim();
    let count_err = Error {
        fragment,
        message: "expected two arguments",
    };
    if fragment.len() == 0 {
        Err(Error {
            fragment: opcode,
            message: count_err.message,
        })
    } else if let (a, Some((c, b))) = fragment.split_on(',') {
        if let (_, Some(_)) = b.split_on(',') {
            Err(count_err)
        } else {
            let a = A::parse(a).map_err(|e| e.or(Error {
                fragment: c,
                message: "missing argument",
            }))?;
            let b = B::parse(b).map_err(|e| e.or(Error {
                fragment: c,
                message: "missing argument",
            }))?;
            Ok(instruction(a, b))
        }
    } else {
        Err(count_err)
    }
}

fn parse3<'a, A: ParseArg<'a>, B: ParseArg<'a>, C: ParseArg<'a>, F: FnOnce(A, B, C) -> Instruction<'a>>(
    fragment: Fragment<'a>,
    opcode: Fragment<'a>,
    instruction: F,
) -> Result<Instruction<'a>, Error<'a>> {
    let fragment = fragment.trim();
    let count_err = Error {
        fragment,
        message: "expected two argument",
    };
    if fragment.len() == 0 {
        Err(Error {
            fragment: opcode,
            message: count_err.message,
        })
    } else if let (a, Some((c1, bb))) = fragment.split_on(',') {
        if let (b, Some((c2, c))) = bb.split_on(',') {
            if let (_, Some(_)) = c.split_on(',') {
                Err(count_err)
            } else {
                let a = A::parse(a).map_err(|e| e.or(Error {
                    fragment: c1,
                    message: "missing argument",
                }))?;
                let b = B::parse(b).map_err(|e| e.or(Error {
                    fragment: c1,
                    message: "missing argument",
                }))?;
                let c = C::parse(c).map_err(|e| e.or(Error {
                    fragment: c2,
                    message: "missing argument",
                }))?;
                Ok(instruction(a, b, c))
            }
        } else {
            Err(count_err)
        }
    } else {
        Err(count_err)
    }
}

fn parse_data<'a>(args: Fragment<'a>, opcode: Fragment<'a>) -> Result<Instruction<'a>, Error<'a>> {
    let mut bytes = Vec::new();
    let mut args = args.trim();
    let mut if_missing = opcode;
    let last = loop {
        if let (a, Some((c, rest))) = args.split_on(',') {
            let a = a.trim();
            if a.len() == 0 {
                return Err(Error {
                    fragment: c,
                    message: "missing byte",
                });
            }
            let num = Number::parse(a)?;
            if num.value >= 256 {
                return Err(Error {
                    fragment: a,
                    message: "byte value too large",
                });
            }
            bytes.push(num);
            if_missing = c;
            args = rest;
        } else {
            break args;
        }
    };
    let last = last.trim();
    if last.len() == 0 {
        return Err(Error {
            fragment: if_missing,
            message: "missing byte",
        });
    }
    let num = Number::parse(last)?;
    if num.value >= 256 {
        return Err(Error {
            fragment: last,
            message: "byte value too large",
        });
    }
    bytes.push(num);
    Ok(Instruction::Bytes(bytes))
}

fn matches(fragment: Fragment<'_>, s: &str) -> bool {
    let mut fc = fragment.as_str().chars().flat_map(char::to_lowercase);
    let sc = s.chars().flat_map(char::to_lowercase);
    for c in sc {
        if fc.next() != Some(c) {
            return false;
        }
    }
    fc.next().is_none()
}

fn parse_instruction<'a>(fragment: Fragment<'a>) -> Result<Option<Instruction<'a>>, Error<'a>> {
    let fragment = fragment.trim();
    if fragment.len() == 0 {
        return Ok(None);
    }
    let (opcode, args) = fragment.split_on(' ');
    let opcode = opcode.trim();
    let args = args.map(|(_, a)| a).unwrap_or(fragment.prefix(0));
    Ok(Some(match opcode {
        o if matches(o, "nop") => parse0(args, opcode, Instruction::Nop)?,
        o if matches(o, "ret") => parse0(args, opcode, Instruction::Ret)?,
        o if matches(o, "wait") => parse0(args, opcode, Instruction::Wait)?,
        o if matches(o, "poll") => parse0(args, opcode, Instruction::Poll)?,
        o if matches(o, "not") => parse1(args, opcode, Instruction::Not)?,
        o if matches(o, "neg") => parse1(args, opcode, Instruction::Neg)?,
        o if matches(o, "pop") => parse1(args, opcode, Instruction::Pop)?,
        o if matches(o, "push") => parse1(args, opcode, Instruction::Push)?,
        o if matches(o, "jmp") => parse1(args, opcode, Instruction::Jmp)?,
        o if matches(o, "call") => parse1(args, opcode, Instruction::Call)?,
        o if matches(o, "mov") => parse2(args, opcode, Instruction::Mov)?,
        o if matches(o, "add") => parse2(args, opcode, Instruction::Add)?,
        o if matches(o, "sub") => parse2(args, opcode, Instruction::Sub)?,
        o if matches(o, "xor") => parse2(args, opcode, Instruction::Xor)?,
        o if matches(o, "and") => parse2(args, opcode, Instruction::And)?,
        o if matches(o, "or") => parse2(args, opcode, Instruction::Or)?,
        o if matches(o, "shl") => parse2(args, opcode, Instruction::Shl)?,
        o if matches(o, "shr") => parse2(args, opcode, Instruction::Shr)?,
        o if matches(o, "cmp") => parse2(args, opcode, Instruction::Cmp)?,
        o if matches(o, "load") => parse2(args, opcode, Instruction::Load)?,
        o if matches(o, "loadw") => parse2(args, opcode, Instruction::Loadw)?,
        o if matches(o, "store") => parse2(args, opcode, Instruction::Store)?,
        o if matches(o, "storew") => parse2(args, opcode, Instruction::Storew)?,
        o if matches(o, "jez") => parse2(args, opcode, Instruction::Jez)?,
        o if matches(o, "jnz") => parse2(args, opcode, Instruction::Jnz)?,
        o if matches(o, "jl") => parse2(args, opcode, Instruction::Jl)?,
        o if matches(o, "jg") => parse2(args, opcode, Instruction::Jg)?,
        o if matches(o, "jle") => parse2(args, opcode, Instruction::Jle)?,
        o if matches(o, "jge") => parse2(args, opcode, Instruction::Jge)?,
        o if matches(o, "read") => parse3(args, opcode, Instruction::Read)?,
        o if matches(o, "write") => parse3(args, opcode, Instruction::Write)?,
        o if matches(o, "db") => parse_data(args, opcode)?,
        _ => return Err(Error {
            fragment: opcode,
            message: "invalid instruction",
        }),
    }))
}

fn parse_line<'a>(fragment: Fragment<'a>) -> Result<Line<'a>, Error<'a>> {
    let fragment = fragment.split_on(';').0;
    match fragment.split_on(':') {
        (label, Some((colon, instruction))) => {
            let label = label.trim();
            if label.len() == 0 {
                Err(Error {
                    fragment: colon,
                    message: "missing label",
                })
            } else if !is_identifier(label.as_str()) {
                Err(Error {
                    fragment: label,
                    message: "invalid label",
                })
            } else {
                Ok(Line {
                    label: Some(label.as_str()),
                    instruction: parse_instruction(instruction)?,
                })
            }
        }
        (instruction, None) => Ok(Line {
            label: None,
            instruction: parse_instruction(instruction)?,
        }),
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        eprintln!("usage: {} <input> <output>", args[0]);
        std::process::exit(2);
    }
    let source = match std::fs::read_to_string(&args[1]) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {}:\n  {}", args[1], e);
            std::process::exit(1);
        }
    };
    let mut lines = Vec::new();
    let mut all_ok = true;
    for (index, line) in source.lines().enumerate() {
        let index = index + 1;
        match parse_line(Fragment::new(line, index)) {
            Ok(line) => lines.push(line),
            Err(e) => {
                all_ok = false;
                eprintln!("{}", e);
            }
        }
    }
    if !all_ok {
        std::process::exit(1);
    }
    let mut labels = HashMap::new();
    for line in &lines {
        if let Some(label) = line.label {
            labels.insert(label, 0);
        }
    }
    let mut output_bytes = Vec::new();
    for line in &lines {
        if let Some(label) = line.label {
            labels.insert(label, output_bytes.len() as u16);
        }
        if let Some(instruction) = &line.instruction {
            match instruction.assemble(&mut output_bytes, &labels) {
                Ok(()) => {}
                Err(e) => {
                    all_ok = false;
                    eprintln!("{}", e);
                }
            }
        }
    }
    if !all_ok {
        std::process::exit(1);
    }
    let mut output = Vec::new();
    for line in &lines {
        if let Some(instruction) = &line.instruction {
            if let Err(e) = instruction.assemble(&mut output, &labels) {
                panic!("unexpected error:\n{}", e);
            }
        }
    }

    let mut out_file = match std::fs::File::create(&args[2]) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("failed to create {}:\n  {}", args[2], e);
            std::process::exit(1);
        }
    };

    if let Err(e) = out_file.write_all(&output) {
        eprintln!("failed to write {}:\n  {}", args[2], e);
        std::process::exit(1);
    }
}
