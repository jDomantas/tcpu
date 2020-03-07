#![cfg_attr(all(not(test), not(feature = "std")), no_std)]

#[cfg(test)]
mod tests;
#[cfg(any(test, feature = "std"))]
mod std_impls;

use core::fmt;

pub const DISK_SIZE: usize = 1 << 20;
pub const MEMORY_SIZE: usize = 1 << 16;

pub const SCREEN_WIDTH: usize = 128;
pub const SCREEN_HEIGHT: usize = 96;

const SCREEN_POSITION: u16 = 0b1100_0000_0000_0000;
const SCREEN_REFRESH_TIME: u64 = 78643;
const DISK_OP_SIZE: usize = 4096;
const DISK_OP_EXTEND: u64 = 1_572_864 / 2;
const CYCLES_PER_BYTE: u64 = 32;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
struct Event {
    id: u16,
    arg: u16,
}

impl Event {
    fn key_up(key: u16) -> Self {
        Event { id: 1, arg: key }
    }

    fn key_down(key: u16) -> Self {
        Event { id: 2, arg: key }
    }

    fn screen_refresh() -> Self {
        Event { id: 3, arg: 0 }
    }

    fn disk_finished(disk: DiskId, result: DiskResult) -> Self {
        let id = match disk {
            DiskId::D0 => 4,
            DiskId::D1 => 5,
        };
        let arg = match result {
            DiskResult::Ok => 0,
            DiskResult::DiskNotPresent => 1,
            DiskResult::DiskBusy => 2,
        };
        Event { id, arg }
    }
}

const EVENT_QUEUE_CAPACITY: usize = 64;

#[derive(Clone)]
struct EventQueue {
    items: [Event; EVENT_QUEUE_CAPACITY],
    head: usize,
    len: usize,
}

impl EventQueue {
    const fn new() -> Self {
        EventQueue {
            items: [Event { id: 0, arg: 0 }; EVENT_QUEUE_CAPACITY],
            head: 0,
            len: 0,
        }
    }

    fn push(&mut self, event: Event) {
        self.items[(self.head + self.len) % EVENT_QUEUE_CAPACITY] = event;
        if self.len == EVENT_QUEUE_CAPACITY {
            self.head = (self.head + 1) % EVENT_QUEUE_CAPACITY;
        } else {
            self.len += 1;
        }
    }

    fn pop(&mut self) -> Option<Event> {
        if self.len == 0 {
            None
        } else {
            // use index modulo capacity so that
            // bounds check would be optimized out
            let event = self.items[self.head % EVENT_QUEUE_CAPACITY];
            self.head = (self.head + 1) % EVENT_QUEUE_CAPACITY;
            self.len -= 1;
            Some(event)
        }
    }
}

enum DiskOp {
    Reading {
        disk_ptr: usize,
        memory_ptr: usize,
        remaining: usize,
        delay: u64,
    },
    Writing {
        disk_ptr: usize,
        memory_ptr: usize,
        remaining: usize,
        delay: u64,
    },
}

#[derive(PartialEq, Eq, Debug, Hash, Copy, Clone)]
#[non_exhaustive]
pub enum DiskId {
    D0,
    D1,
}

const DISK_IDS: &[DiskId] = &[DiskId::D0, DiskId::D1];

enum DiskResult {
    Ok,
    DiskNotPresent,
    DiskBusy,
}

pub struct DiskIdConvertError;

macro_rules! disk_id_from {
    ($($num:tt),*) => {
        $(
            impl core::convert::TryFrom<$num> for DiskId {
                type Error = DiskIdConvertError;
            
                fn try_from(from: $num) -> Result<Self, Self::Error> {
                    match from {
                        0 => Ok(DiskId::D0),
                        1 => Ok(DiskId::D1),
                        _ => Err(DiskIdConvertError),
                    }
                }
            }
        )*
    }
}

disk_id_from!(i8, u8, i16, u16, i32, u32, i64, u64, i128, u128, isize, usize);

pub trait Storage<T> {
    fn as_ref(&self) -> &T;
    fn as_mut(&mut self) -> &mut T;
}

impl Storage<[u8; MEMORY_SIZE]> for [u8; MEMORY_SIZE] {
    fn as_ref(&self) -> &[u8; MEMORY_SIZE] { self }
    fn as_mut(&mut self) -> &mut [u8; MEMORY_SIZE] { self }
}

impl Storage<[u8; DISK_SIZE]> for [u8; DISK_SIZE] {
    fn as_ref(&self) -> &[u8; DISK_SIZE] { self }
    fn as_mut(&mut self) -> &mut [u8; DISK_SIZE] { self }
}

pub struct Disk<S> {
    pub data: S,
    pub modified: bool,
    pub running: bool,
}

struct PluggedDisk<S> {
    disk: Disk<S>,
    running_remaining: u64,
    running_op: Option<DiskOp>,
}

#[derive(PartialEq, Eq)]
enum CpuState {
    Running,
    Waiting,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Registers {
    pub a: u16,
    pub b: u16,
    pub c: u16,
    pub d: u16,
    pub i: u16,
    pub j: u16,
    pub p: u16,
    pub s: u16,
}

impl Registers {
    pub const fn new() -> Self {
        Registers {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            i: 0,
            j: 0,
            p: 0,
            s: 0,
        }
    }

    pub fn get(&self, reg: Register) -> u16 {
        match reg {
            Register::A => self.a,
            Register::B => self.b,
            Register::C => self.c,
            Register::D => self.d,
            Register::I => self.i,
            Register::J => self.j,
            Register::P => self.p,
            Register::S => self.s,
        }
    }

    fn set(&mut self, reg: Register, value: u16) {
        *self.get_mut(reg) = value;
    }
    
    pub fn get_mut(&mut self, reg: Register) -> &mut u16 {
        match reg {
            Register::A => &mut self.a,
            Register::B => &mut self.b,
            Register::C => &mut self.c,
            Register::D => &mut self.d,
            Register::I => &mut self.i,
            Register::J => &mut self.j,
            Register::P => &mut self.p,
            Register::S => &mut self.s,
        }
    }
}

pub trait Tracer {
    fn on_screen_refresh(&self, _screen: &[[u8; SCREEN_WIDTH]; SCREEN_HEIGHT]) {}
    fn register_values(&self, _values: Registers) {}
    fn on_instruction(&self, _address: u16, _instruction: Instruction) {}
    fn on_load(&self, _address: u16, _value: u16, _wide: bool) {}
    fn on_store(&self, _address: u16, _value: u16, _wide: bool) {}
}

pub struct NoopTracer;

impl Tracer for NoopTracer {}

pub struct Emulator<SM, SD, T = NoopTracer> {
    tracer: T,
    memory: SM,
    screen: [[u8; SCREEN_WIDTH]; SCREEN_HEIGHT],
    registers: Registers,
    instruction_pointer: u16,
    event_queue: EventQueue,
    disks: [Option<PluggedDisk<SD>>; 2],
    cycles: u64,
    time_to_refresh: u64,
    state: CpuState,
}

impl<SM, SD> Emulator<SM, SD, NoopTracer> {
    pub const fn new(memory: SM) -> Self {
        Self::with_tracer(memory, NoopTracer)
    }
}

impl<SM, SD, T> Emulator<SM, SD, T> {
    pub const fn with_tracer(memory: SM, tracer: T) -> Self {
        Emulator {
            tracer,
            memory,
            screen: [[0; SCREEN_WIDTH]; SCREEN_HEIGHT],
            registers: Registers::new(),
            instruction_pointer: 0,
            event_queue: EventQueue::new(),
            disks: [None, None],
            cycles: 0,
            time_to_refresh: SCREEN_REFRESH_TIME,
            state: CpuState::Running,
        }
    }
}

impl<SM, SD, T> Emulator<SM, SD, T>
where
    SM: Storage<[u8; MEMORY_SIZE]>,
    SD: Storage<[u8; DISK_SIZE]>,
    T: Tracer,
{
    fn queue_event(&mut self, event: Event) {
        self.event_queue.push(event);
    }

    fn disk_slot_mut(&mut self, id: DiskId) -> &mut Option<PluggedDisk<SD>> {
        match id {
            DiskId::D0 => &mut self.disks[0],
            DiskId::D1 => &mut self.disks[1],
        }
    }

    pub fn plug_disk(&mut self, id: DiskId, disk: Disk<SD>) -> Option<Disk<SD>> {
        let old = self.unplug_disk(id);
        *self.disk_slot_mut(id) = Some(PluggedDisk {
            disk,
            running_remaining: 0,
            running_op: None,
        });
        old
    }

    pub fn unplug_disk(&mut self, id: DiskId) -> Option<Disk<SD>> {
        self.disk_slot_mut(id).take().map(|s| s.disk)
    }

    pub fn disk(&self, id: DiskId) -> Option<&Disk<SD>> {
        match id {
            DiskId::D0 => self.disks[0].as_ref().map(|s| &s.disk),
            DiskId::D1 => self.disks[1].as_ref().map(|s| &s.disk),
        }
    }

    pub fn screen(&self) -> &[[u8; SCREEN_WIDTH]; SCREEN_HEIGHT] {
        &self.screen
    }

    pub fn memory_mut(&mut self) -> &mut [u8; MEMORY_SIZE] {
        self.memory.as_mut()
    }

    pub fn reset(&mut self) {
        let mut disks = [self.disks[0].take(), self.disks[1].take()];
        for disk in &mut disks {
            if let Some(disk) = disk {
                disk.running_op = None;
            }
        }
        for byte in &mut self.memory.as_mut()[..] {
            *byte = 0;
        }
        self.screen = [[0; SCREEN_WIDTH]; SCREEN_HEIGHT];
        self.registers = Registers::default();
        self.instruction_pointer = 0;
        self.event_queue = EventQueue::new();
        self.disks = disks;
        self.cycles = 0;
        self.time_to_refresh = SCREEN_REFRESH_TIME;
        self.state = CpuState::Running;
        if let Some(disk) = &mut self.disks[0] {
            self.memory.as_mut()[..DISK_OP_SIZE].copy_from_slice(
                &disk.disk.data.as_ref()[..DISK_OP_SIZE],
            );
            disk.running_remaining = DISK_OP_EXTEND;
            disk.disk.running = true;
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.load(self.instruction_pointer);
        self.instruction_pointer = self.instruction_pointer.wrapping_add(1);
        byte
    }

    fn read_word(&mut self) -> u16 {
        let low = u16::from(self.read_byte());
        let high = u16::from(self.read_byte());
        (high << 8) + low
    }

    fn refresh_screen(&mut self) {
        for row in 0..SCREEN_HEIGHT {
            for col in 0..SCREEN_WIDTH {
                let offset = row * SCREEN_WIDTH + col;
                let addr = SCREEN_POSITION + (offset as u16);
                self.screen[row][col] = self.load(addr);
            }
        }
        self.tracer.on_screen_refresh(&self.screen);
    }
    
    fn update_disk(
        disk_id: DiskId,
        memory: &mut [u8; MEMORY_SIZE],
        disk: &mut PluggedDisk<SD>,
    ) -> Option<Event> {
        if disk.running_op.is_some() {
            disk.running_remaining = DISK_OP_EXTEND;
        } else if disk.running_remaining > 0 {
            disk.running_remaining -= 1;
        }
        disk.disk.running = disk.running_remaining > 0;
        match &mut disk.running_op {
            Some(DiskOp::Reading { disk_ptr, memory_ptr, remaining, delay }) => {
                *delay -= 1;
                if *delay == 0 {
                    *delay = CYCLES_PER_BYTE;
                    memory[*memory_ptr % memory.len()] = disk.disk.data.as_ref()[*disk_ptr % DISK_SIZE];
                    *disk_ptr = disk_ptr.wrapping_add(1);
                    *memory_ptr = memory_ptr.wrapping_add(1);
                    *remaining -= 1;
                    if *remaining == 0 {
                        disk.running_op = None;
                        Some(Event::disk_finished(disk_id, DiskResult::Ok))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Some(DiskOp::Writing { disk_ptr, memory_ptr, remaining, delay }) => {
                *delay -= 1;
                if *delay == 0 {
                    *delay = CYCLES_PER_BYTE;
                    disk.disk.data.as_mut()[*disk_ptr % DISK_SIZE] = memory[*memory_ptr % memory.len()];
                    *disk_ptr = disk_ptr.wrapping_add(1);
                    *memory_ptr = memory_ptr.wrapping_add(1);
                    *remaining -= 1;
                    if *remaining == 0 {
                        disk.running_op = None;
                        Some(Event::disk_finished(disk_id, DiskResult::Ok))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            None => None
        }
    }

    pub fn cycle(&mut self) {
        if self.time_to_refresh == 0 {
            self.refresh_screen();
            self.time_to_refresh = SCREEN_REFRESH_TIME;
            self.event_queue.push(Event::screen_refresh());
        }

        self.time_to_refresh -= 1;

        for (index, &disk_id) in DISK_IDS.iter().enumerate() {
            if let Some(disk) = &mut self.disks[index] {
                if let Some(event) = Self::update_disk(
                    disk_id,
                    self.memory.as_mut(),
                    disk,
                ) {
                    self.queue_event(event);
                }
            }
        }

        self.cycles += 1;
        
        if self.state == CpuState::Waiting {
            if let Some(event) = self.event_queue.pop() {
                self.registers.a = event.id;
                self.registers.b = event.arg;
                self.state = CpuState::Running;
            }
        }
        
        if self.state == CpuState::Running {
            self.tracer.register_values(self.registers);
            let address = self.instruction_pointer;
            let instruction = self.decode_instruction();
            self.tracer.on_instruction(address, instruction);
            self.apply_instruction(instruction);
        }
    }

    pub fn run(&mut self, cycles: u64) {
        for _ in 0..cycles {
            self.cycle();
        }
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    fn decode_instruction(&mut self) -> Instruction {
        let x = self.read_byte();
        match x {
            0b0000_0000 => Instruction::Nop,
            0b0000_0001 => Instruction::Ret,
            0b0000_0010 => Instruction::Wait,
            0b0000_0011 => Instruction::Poll,
            0b1000_0000 => self.register_operand(Instruction::Mov),
            0b1000_0001 => self.register_operand(Instruction::Add),
            0b1000_0010 => self.register_operand(Instruction::Sub),
            0b1000_0011 => self.register_operand(Instruction::Xor),
            0b1000_0100 => self.register_operand(Instruction::And),
            0b1000_0101 => self.register_operand(Instruction::Or),
            0b1000_0110 => self.register_operand(Instruction::Shl),
            0b1000_0111 => self.register_operand(Instruction::Shr),
            0b1000_1000 => self.register_operand(Instruction::Cmp),
            0b1001_0000 ..= 0b1001_0010 => self.decode_load(x, Instruction::Load),
            0b1001_0100 ..= 0b1001_0110 => self.decode_load(x, Instruction::Loadw),
            0b1001_1000 ..= 0b1001_1010 => self.decode_store(x, Instruction::Store),
            0b1001_1100 ..= 0b1001_1110 => self.decode_store(x, Instruction::Storew),
            0b0001_0000 ..= 0b0001_0111 => self.one_register(x, Instruction::Not),
            0b0010_0000 ..= 0b0010_0111 => self.one_register(x, Instruction::Neg),
            0b0011_0000 ..= 0b0011_0111 => self.one_register(x, Instruction::Pop),
            0b0100_0000 ..= 0b0100_1111 => self.one_operand(x, Instruction::Push),
            0b0101_0000 ..= 0b0101_1111 => self.one_operand(x, Instruction::Jmp),
            0b0110_0000 ..= 0b0110_1111 => self.one_operand(x, Instruction::Call),
            0b1010_0000 => self.register_operand(Instruction::Jez),
            0b1010_0001 => self.register_operand(Instruction::Jnz),
            0b1010_0010 => self.register_operand(Instruction::Jl),
            0b1010_0011 => self.register_operand(Instruction::Jg),
            0b1010_0100 => self.register_operand(Instruction::Jle),
            0b1010_0101 => self.register_operand(Instruction::Jge),
            0b1111_0000 => self.two_operands(|a, b| Instruction::Read(DiskId::D0, a, b)),
            0b1111_0001 => self.two_operands(|a, b| Instruction::Read(DiskId::D1, a, b)),
            0b1111_1000 => self.two_operands(|a, b| Instruction::Write(DiskId::D0, a, b)),
            0b1111_1001 => self.two_operands(|a, b| Instruction::Write(DiskId::D1, a, b)),
            _ => Instruction::Invalid,
        }
    }

    fn one_register(&mut self, x: u8, f: impl FnOnce(Register) -> Instruction) -> Instruction {
        let register = self.decode_register(x & 0b111);
        f(register)
    }

    fn one_operand(&mut self, x: u8, f: impl FnOnce(Operand) -> Instruction) -> Instruction {
        let operand = self.decode_operand(x & 0b1111);
        f(operand)
    }

    fn register_operand(&mut self, f: impl FnOnce(Register, Operand) -> Instruction) -> Instruction {
        let x = self.read_byte();
        let reg = self.decode_register((x >> 4) & 0b111);
        let op = self.decode_operand(x & 0b1111);
        f(reg, op)
    }

    fn two_operands(&mut self, f: impl FnOnce(Operand, Operand) -> Instruction) -> Instruction {
        let x = self.read_byte();
        let op1 = self.decode_operand((x >> 4) & 0b1111);
        let op2 = self.decode_operand(x & 0b1111);
        f(op1, op2)
    }

    fn decode_load(&mut self, b: u8, f: impl FnOnce(Register, Address) -> Instruction) -> Instruction {
        let x = self.read_byte();
        let offset = match b & 0b11 {
            1 => u16::from(self.read_byte()),
            2 => self.read_word(),
            _ => 0,
        };
        let reg = self.decode_register((x >> 4) & 0b111);
        let operand = self.decode_operand(x & 0b1111);
        f(reg, Address { operand, offset })
    }

    fn decode_store(&mut self, b: u8, f: impl FnOnce(Operand, Address) -> Instruction) -> Instruction {
        let x = self.read_byte();
        let offset = match b & 0b11 {
            1 => u16::from(self.read_byte()),
            2 => self.read_word(),
            _ => 0,
        };
        let op = self.decode_operand((x >> 4) & 0b1111);
        let operand = self.decode_operand(x & 0b1111);
        f(op, Address { operand, offset })
    }

    fn load(&self, addr: u16) -> u8 {
        self.memory.as_ref()[addr as usize]
    }

    fn load_word(&self, addr: u16) -> u16 {
        let low = self.memory.as_ref()[addr as usize];
        let high = self.memory.as_ref()[addr.wrapping_add(1) as usize];
        u16::from_le_bytes([low, high])
    }

    fn store(&mut self, addr: u16, value: u8) {
        self.memory.as_mut()[addr as usize] = value;
    }

    fn store_word(&mut self, addr: u16, value: u16) {
        let [low, high] = value.to_le_bytes();
        self.memory.as_mut()[addr as usize] = low;
        self.memory.as_mut()[addr.wrapping_add(1) as usize] = high;
    }

    fn apply_instruction(&mut self, instruction: Instruction) {
        match instruction {
            Instruction::Nop => {}
            Instruction::Ret => {
                self.registers.s = self.registers.s.wrapping_add(2);
                self.instruction_pointer = self.load_word(self.registers.s);
            }
            Instruction::Wait => self.state = CpuState::Waiting,
            Instruction::Poll => {
                if let Some(event) = self.event_queue.pop() {
                    self.registers.a = event.id;
                    self.registers.b = event.arg;
                } else {
                    self.registers.a = 0;
                    self.registers.b = 0;
                }
            }
            Instruction::Not(a) => self.registers.set(a, !self.eval(a)),
            Instruction::Neg(a) => self.registers.set(a, self.eval(a).wrapping_neg()),
            Instruction::Pop(a) => {
                self.registers.s = self.registers.s.wrapping_add(2);
                self.registers.set(a, self.load_word(self.registers.s));
            }
            Instruction::Push(a) => {
                self.store_word(self.registers.s, self.eval(a));
                self.registers.s = self.registers.s.wrapping_sub(2);
            }
            Instruction::Jmp(a) => self.instruction_pointer = self.eval(a),
            Instruction::Call(a) => {
                self.store_word(self.registers.s, self.instruction_pointer);
                self.instruction_pointer = self.eval(a);
                self.registers.s = self.registers.s.wrapping_sub(2);
            }
            Instruction::Mov(a, b) => self.registers.set(a, self.eval(b)),
            Instruction::Add(a, b) => self.registers.set(a, self.eval(a).wrapping_add(self.eval(b))),
            Instruction::Sub(a, b) => self.registers.set(a, self.eval(a).wrapping_sub(self.eval(b))),
            Instruction::Xor(a, b) => self.registers.set(a, self.eval(a) ^ self.eval(b)),
            Instruction::And(a, b) => self.registers.set(a, self.eval(a) & self.eval(b)),
            Instruction::Or(a, b) => self.registers.set(a, self.eval(a) | self.eval(b)),
            Instruction::Shl(a, b) => self.registers.set(a, {
                let shift = self.eval(b);
                if shift >= 16 {
                    0
                } else {
                    self.eval(a) << shift
                }
            }),
            Instruction::Shr(a, b) => self.registers.set(a, {
                let shift = self.eval(b);
                if shift >= 16 {
                    0
                } else {
                    self.eval(a) >> shift
                }
            }),
            Instruction::Cmp(a, b) => {
                let av = self.eval(a);
                let bv = self.eval(b);
                self.registers.set(a, if av > bv {
                    1
                } else if av == bv {
                    0
                } else {
                    0xffff
                })
            }
            Instruction::Load(a, b) => {
                let addr = self.eval(b);
                let value = u16::from(self.load(addr));
                self.tracer.on_load(addr, value, false);
                self.registers.set(a, value);
            }
            Instruction::Loadw(a, b) => {
                let addr = self.eval(b);
                let value = self.load_word(addr);
                self.tracer.on_load(addr, value, true);
                self.registers.set(a, value);
            }
            Instruction::Store(a, b) => {
                let addr = self.eval(b);
                let value = self.eval(a).to_le_bytes()[0];
                self.tracer.on_store(addr, u16::from(value), false);
                self.store(addr, value);
            }
            Instruction::Storew(a, b) => {
                let addr = self.eval(b);
                let value = self.eval(a);
                self.tracer.on_store(addr, value, true);
                self.store_word(addr, value);
            }
            Instruction::Jez(a, d) => {
                if self.eval(a) == 0 {
                    self.instruction_pointer = self.eval(d);
                }
            }
            Instruction::Jnz(a, d) => {
                if self.eval(a) != 0 {
                    self.instruction_pointer = self.eval(d);
                }
            }
            Instruction::Jl(a, d) => {
                if self.eval(a) == 0xffff {
                    self.instruction_pointer = self.eval(d);
                }
            }
            Instruction::Jg(a, d) => {
                if self.eval(a) == 1 {
                    self.instruction_pointer = self.eval(d);
                }
            }
            Instruction::Jle(a, d) => {
                if self.eval(a) != 1 {
                    self.instruction_pointer = self.eval(d);
                }
            }
            Instruction::Jge(a, d) => {
                if self.eval(a) != 0xffff {
                    self.instruction_pointer = self.eval(d);
                }
            }
            Instruction::Read(id, memory_ptr, disk_ptr) => {
                let memory_ptr = usize::from(self.eval(memory_ptr));
                let disk_ptr = usize::from(self.eval(disk_ptr)) * 16;
                if let Some(disk) = self.disk_slot_mut(id) {
                    if disk.running_op.is_none() {
                        disk.running_op = Some(DiskOp::Reading {
                            memory_ptr,
                            disk_ptr,
                            remaining: DISK_OP_SIZE,
                            delay: CYCLES_PER_BYTE,
                        });
                    } else {
                        self.queue_event(Event::disk_finished(id, DiskResult::DiskBusy));
                    }
                } else {
                    self.queue_event(Event::disk_finished(id, DiskResult::DiskNotPresent));
                }
            }
            Instruction::Write(id, memory_ptr, disk_ptr) => {
                let memory_ptr = usize::from(self.eval(memory_ptr));
                let disk_ptr = usize::from(self.eval(disk_ptr)) * 16;
                if let Some(disk) = self.disk_slot_mut(id) {
                    if disk.running_op.is_none() {
                        disk.disk.modified = true;
                        disk.running_op = Some(DiskOp::Writing {
                            memory_ptr,
                            disk_ptr,
                            remaining: DISK_OP_SIZE,
                            delay: CYCLES_PER_BYTE,
                        });
                    } else {
                        self.queue_event(Event::disk_finished(id, DiskResult::DiskBusy));
                    }
                } else {
                    self.queue_event(Event::disk_finished(id, DiskResult::DiskNotPresent));
                }
            }
            Instruction::Invalid => {}
        }
    }
    
    fn decode_register(&mut self, bits: u8) -> Register {
        match bits & 0b111 {
            0b000 => Register::A,
            0b001 => Register::B,
            0b010 => Register::C,
            0b011 => Register::D,
            0b100 => Register::I,
            0b101 => Register::J,
            0b110 => Register::P,
            0b111 => Register::S,
            _ => unreachable!(),
        }
    }
    
    fn decode_operand(&mut self, bits: u8) -> Operand {
        match bits & 0b1111 {
            0b0000 ..= 0b0111 => Operand::Register(self.decode_register(bits)),
            0b1000 => Operand::Word(0),
            0b1001 => Operand::Word(1),
            0b1010 => Operand::Word(2),
            0b1011 => Operand::Word(3),
            0b1100 => Operand::Word(4),
            0b1101 => Operand::Word(u16::from(self.read_byte())),
            0b1110 => Operand::Word(self.read_word()),
            0b1111 => Operand::Word(0xffff),
            _ => unreachable!(),
        }
    }

    pub fn key_up(&mut self, key: u16) {
        self.queue_event(Event::key_up(key));
    }

    pub fn key_down(&mut self, key: u16) {
        self.queue_event(Event::key_down(key));
    }
}

trait Eval<T> {
    fn eval(&self, expr: T) -> u16;
}

#[derive(Debug, Copy, Clone)]
pub enum Register {
    A,
    B,
    C,
    D,
    I,
    J,
    P,
    S,
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Register::A => "A",
            Register::B => "B",
            Register::C => "C",
            Register::D => "D",
            Register::I => "I",
            Register::J => "J",
            Register::P => "P",
            Register::S => "S",
        };
        write!(f, "{}", s)
    }
}

impl<SM, SD, T> Eval<Register> for Emulator<SM, SD, T> {
    fn eval(&self, register: Register) -> u16 {
        self.registers.get(register)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Operand {
    Register(Register),
    Word(u16),
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Operand::Register(r) => write!(f, "{}", r),
            Operand::Word(w) => write!(f, "{}", w),
        }
    }
}

impl<SM, SD, T> Eval<Operand> for Emulator<SM, SD, T> {
    fn eval(&self, operand: Operand) -> u16 {
        match operand {
            Operand::Register(r) => self.eval(r),
            Operand::Word(w) => w,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Address {
    pub operand: Operand,
    pub offset: u16,
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.offset == 0 {
            write!(f, "{}", self.operand)
        } else {
            write!(f, "{} + {}", self.operand, self.offset)
        }
    }
}

impl<SM, SD, T> Eval<Address> for Emulator<SM, SD, T> {
    fn eval(&self, address: Address) -> u16 {
        self.eval(address.operand).wrapping_add(address.offset)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Instruction {
    Nop,
    Ret,
    Wait,
    Poll,
    Not(Register),
    Neg(Register),
    Pop(Register),
    Push(Operand),
    Jmp(Operand),
    Call(Operand),
    Mov(Register, Operand),
    Add(Register, Operand),
    Sub(Register, Operand),
    Xor(Register, Operand),
    And(Register, Operand),
    Or(Register, Operand),
    Shl(Register, Operand),
    Shr(Register, Operand),
    Cmp(Register, Operand),
    Load(Register, Address),
    Loadw(Register, Address),
    Store(Operand, Address),
    Storew(Operand, Address),
    Jez(Register, Operand),
    Jnz(Register, Operand),
    Jl(Register, Operand),
    Jg(Register, Operand),
    Jle(Register, Operand),
    Jge(Register, Operand),
    Read(DiskId, Operand, Operand),
    Write(DiskId, Operand, Operand),
    Invalid,
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Instruction::Nop => write!(f, "nop"),
            Instruction::Ret => write!(f, "ret"),
            Instruction::Wait => write!(f, "wait"),
            Instruction::Poll => write!(f, "poll"),
            Instruction::Not(a) => write!(f, "not {}", a),
            Instruction::Neg(a) => write!(f, "neg {}", a),
            Instruction::Pop(a) => write!(f, "pop {}", a),
            Instruction::Push(a) => write!(f, "push {}", a),
            Instruction::Jmp(a) => write!(f, "jmp {}", a),
            Instruction::Call(a) => write!(f, "call {}", a),
            Instruction::Mov(a, b) => write!(f, "mov {}, {}", a, b),
            Instruction::Add(a, b) => write!(f, "add {}, {}", a, b),
            Instruction::Sub(a, b) => write!(f, "sub {}, {}", a, b),
            Instruction::Xor(a, b) => write!(f, "xor {}, {}", a, b),
            Instruction::And(a, b) => write!(f, "and {}, {}", a, b),
            Instruction::Or(a, b) => write!(f, "or {}, {}", a, b),
            Instruction::Shl(a, b) => write!(f, "shl {}, {}", a, b),
            Instruction::Shr(a, b) => write!(f, "shr {}, {}", a, b),
            Instruction::Cmp(a, b) => write!(f, "cmp {}, {}", a, b),
            Instruction::Load(a, b) => write!(f, "load {}, {}", a, b),
            Instruction::Loadw(a, b) => write!(f, "loadw {}, {}", a, b),
            Instruction::Store(a, b) => write!(f, "store {}, {}", a, b),
            Instruction::Storew(a, b) => write!(f, "storew {}, {}", a, b),
            Instruction::Jez(x, d) => write!(f, "jez {}, {}", x, d),
            Instruction::Jnz(x, d) => write!(f, "jnz {}, {}", x, d),
            Instruction::Jl(x, d) => write!(f, "jl {}, {}", x, d),
            Instruction::Jg(x, d) => write!(f, "jg {}, {}", x, d),
            Instruction::Jle(x, d) => write!(f, "jle {}, {}", x, d),
            Instruction::Jge(x, d) => write!(f, "jge {}, {}", x, d),
            Instruction::Read(DiskId::D0, a, b) => write!(f, "read0 {}, {}", a, b),
            Instruction::Read(DiskId::D1, a, b) => write!(f, "read1 {}, {}", a, b),
            Instruction::Write(DiskId::D0, a, b) => write!(f, "write0 {}, {}", a, b),
            Instruction::Write(DiskId::D1, a, b) => write!(f, "write1 {}, {}", a, b),
            Instruction::Invalid => write!(f, "???"),
        }
    }
}
