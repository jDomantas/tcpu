use super::*;

fn emulator() -> Emulator<[u8; MEMORY_SIZE], Box<[u8; DISK_SIZE]>, impl Tracer> {
    Emulator::new([0; MEMORY_SIZE])
}

#[test]
fn smoke() {
    let mut emulator = emulator();
    for i in 0..65536 {
        emulator.memory_mut()[i] = i as u8;
    }
    emulator.run(100_000);
}

#[test]
fn mov() {
    let mut emulator = emulator();
    let code = [
        0b1000_0000,
        0b0000_1101,
        58, // mov a, 58
    ];
    for i in 0..code.len() {
        emulator.memory_mut()[i] = code[i];
    }
    emulator.run(1);
    assert_eq!(emulator.registers.a, 58);
}

#[test]
fn stack() {
    let mut emulator = emulator();
    let code = [
        0b0100_1101,
        53,          // push 53
        0b0011_0010, // pop c
    ];
    for i in 0..code.len() {
        emulator.memory_mut()[i] = code[i];
    }
    emulator.run(2);
    assert_eq!(emulator.registers.c, 53);
}

#[test]
fn call() {
    let mut emulator = emulator();
    let code = [
        0x82, 0x7a, // sub s, 2
        0x6d, 0x05, // call 0x05
        0x00, // nop
        0x01, // ret
    ];
    for i in 0..code.len() {
        emulator.memory_mut()[i] = code[i];
    }
    emulator.run(1);
    assert_eq!(emulator.registers.s, 0xfffe);
    emulator.run(1);
    assert_eq!(emulator.memory_mut()[0xffff], 0x00);
    assert_eq!(emulator.memory_mut()[0xfffe], 0x04);
    assert_eq!(emulator.instruction_pointer, 0x05);
    assert_eq!(emulator.registers.s, 0xfffc);
    emulator.run(1);
    assert_eq!(emulator.instruction_pointer, 0x04);
    assert_eq!(emulator.registers.s, 0xfffe);
}

#[test]
fn cmp() {
    let mut emulator = emulator();
    let code = [
        0x80, 0x0a, // mov a, 2
        0x88, 0x09, // cmp a, 1
        0xa5, 0x0f, // jge a, 0xffff
    ];
    for i in 0..code.len() {
        emulator.memory_mut()[i] = code[i];
    }
    emulator.run(1);
    assert_eq!(emulator.registers.a, 0x0002);
    emulator.run(1);
    assert_eq!(emulator.registers.a, 0x0001);
    emulator.run(1);
    assert_eq!(emulator.instruction_pointer, 0xffff);
}

#[test]
fn cmp2() {
    let mut emulator = emulator();
    let code = [
        0x80, 0x0c, // mov a, 4
        0x88, 0x0c, // cmp a, 4
        0xa4, 0x0f, // jle a, 0xffff
    ];
    for i in 0..code.len() {
        emulator.memory_mut()[i] = code[i];
    }
    emulator.run(1);
    assert_eq!(emulator.registers.a, 0x0004);
    emulator.run(1);
    assert_eq!(emulator.registers.a, 0x0000);
    emulator.run(1);
    assert_eq!(emulator.instruction_pointer, 0xffff);
}

#[test]
fn event() {
    let mut emulator = emulator();
    let code = [
        0b0000_0010, // wait
        0b0000_0000, // nop
        0b1000_0011,
        0b0000_0000, // xor a, a
    ];
    for i in 0..code.len() {
        emulator.memory_mut()[i] = code[i];
    }
    emulator.run(10);
    assert_eq!(emulator.registers.a, 0);
    emulator.queue_event(Event { id: 3, arg: 1 });
    emulator.cycle(); // executed nop, registers contain event info
    assert_eq!(emulator.registers.a, 3);
    assert_eq!(emulator.registers.b, 1);
    emulator.cycle(); // executed xor, a is zeroed
    assert_eq!(emulator.registers.a, 0);
}

#[test]
fn event_queue() {
    let mut queue = EventQueue::new();
    assert_eq!(queue.pop(), None);
    queue.push(Event { id: 1, arg: 3 });
    assert_eq!(queue.pop(), Some(Event { id: 1, arg: 3 }));
    assert_eq!(queue.pop(), None);
    queue.push(Event { id: 2, arg: 3 });
    queue.push(Event { id: 1, arg: 5 });
    assert_eq!(queue.pop(), Some(Event { id: 2, arg: 3 }));
    assert_eq!(queue.pop(), Some(Event { id: 1, arg: 5 }));
    assert_eq!(queue.pop(), None);
}
