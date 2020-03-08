use rand::{Rng, SeedableRng};
use std::hash::Hasher;
use super::*;

fn emulator() -> Emulator<Memory, DiskMemory, impl Tracer> {
    Emulator::new()
}

#[derive(Default)]
struct TestCase {
    code: &'static [u8],
    registers: Registers,
    memory: &'static [MemoryUpdate],
}

struct MemoryUpdate {
    address: u16,
    bytes: &'static [u8],
}

fn run_test_case(test: TestCase) {
    let mut emulator = emulator();
    let mut expected_memory = [0; MEMORY_SIZE];
    for (ptr, &byte) in test.code.iter().enumerate() {
        emulator.memory_mut()[ptr] = byte;
        expected_memory[ptr] = byte;
    }
    emulator.memory_mut()[test.code.len()] = 0x04; // halt
    expected_memory[test.code.len()] = 0x04;
    for update in test.memory {
        for (offset, &byte) in update.bytes.iter().enumerate() {
            expected_memory[update.address as usize + offset] = byte;
        }
    }

    emulator.run(100);
    if emulator.state != CpuState::Halted {
        panic!("emulator didn't halt in 100 cycles");
    }
    assert_eq!(test.registers.a, emulator.registers.a, "register a");
    assert_eq!(test.registers.b, emulator.registers.b, "register b");
    assert_eq!(test.registers.c, emulator.registers.c, "register c");
    assert_eq!(test.registers.d, emulator.registers.d, "register d");
    assert_eq!(test.registers.i, emulator.registers.i, "register i");
    assert_eq!(test.registers.j, emulator.registers.j, "register j");
    assert_eq!(test.registers.p, emulator.registers.p, "register p");
    assert_eq!(test.registers.s, emulator.registers.s, "register s");
    assert_eq!(test.code.len() as u16 + 1, emulator.instruction_pointer, "instruction pointer");
    for i in 0..MEMORY_SIZE {
        assert_eq!(expected_memory[i], emulator.memory_mut()[i], "memory value at {}", i);
    }
}

macro_rules! test {
    ($($test_name:ident {
        code: [$($code:tt)*],
        $(registers: { $($registers:tt)* },)?
        $(memory: { $($address:literal: [$($bytes:tt)*]),* $(,)? },)?
    })*) => {
        $(
            #[test]
            fn $test_name() {
                run_test_case(TestCase {
                    code: &[$($code)*],
                    $(registers: Registers { $($registers)* .. Registers::default() },)?
                    $(memory: &[
                        $(MemoryUpdate {
                            address: $address,
                            bytes: &[$($bytes)*],
                        }),*
                    ],)?
                    .. TestCase::default()
                });
            }
        )*
    };
}

#[test]
fn smoke() {
    let mut emulator = emulator();
    for i in 0..65536 {
        emulator.memory_mut()[i] = i as u8;
    }
    emulator.run(100_000);
}

test! {
    nop {
        code: [0, 0, 0, 0],
    }

    ret {
        code: [
            0x4d, 0x06, // push 6
            0x01, // ret
            0x04, // halt
            0x04, // halt
            0x04, // halt
            0x5d, 0x0a, // jmp 0xa
            0x04, // halt
            0x04, // halt
        ],
        memory: {
            0: [0x06, 0x00], // word 0x0006 was pushed when s was 0
        },
    }

    halt {
        code: [],
    }

    not {
        code: [
            0x10, // not a
            0x13, // not d
            0x14, // not i
            0x80, 0x7e, 0x32, 0xd4, // mov s, 0xd432
            0x17, // not s
        ],
        registers: {
            a: 0xffff,
            d: 0xffff,
            i: 0xffff,
            s: !0xd432,
        },
    }

    neg {
        code: [
            0x20, // neg a
            0x80, 0x2e, 0x32, 0xd4, // mov c, 0xd432
            0x22, // neg c
            0x80, 0x32, // mov d, c
            0x23, // neg d
        ],
        registers: {
            c: 0xffff - 0xd432 + 1,
            d: 0xd432,
        },
    }

    jmp {
        code: [
            0x5d, 0x06, // jmp 0x6
            0x04, // halt
            0x04, // halt
            0x04, // halt
            0x04, // halt
        ],
    }

    jmp_indirect {
        code: [
            0x80, 0x5d, 0x07, // mov j, 0x7
            0x55, // jmp j
            0x04, // halt
            0x04, // halt
            0x04, // halt
        ],
        registers: {
            j: 7,
        },
    }

    call {
        code: [
            0x82, 0x7a, // sub s, 2
            0x6d, 0x04, // call 0x04
            0x5d, 0x07, // jmp 0x07
            0x01, // ret
        ],
        registers: {
            s: 0xfffc,
        },
        memory: {
            0xfffe: [0x04],
        },
    }

    mov {
        code: [
            0x80, 0x1d, 0x87, // mov b, 0x87
            0x80, 0x21, // mov c, b
            0x80, 0x3e, 0x34, 0x12, // mov d, 0x1234
            0x80, 0x10, // mov b, a
            0x80, 0x0d, 0xaa, // mov a, 0xaa
            0x80, 0x73, // mov s, d
        ],
        registers: {
            a: 0xaa,
            c: 0x87,
            d: 0x1234,
            s: 0x1234,
        },
    }

    add {
        code: [
            0x81, 0x1d, 0x87, // add b, 0x87
            0x81, 0x21, // add c, b
            0x81, 0x3e, 0x34, 0x12, // add d, 0x1234
            0x81, 0x10, // add b, a
            0x81, 0x0d, 0xaa, // add a, 0xaa
            0x81, 0x73, // add s, d
            0x81, 0x13, // add b, d
        ],
        registers: {
            a: 0xaa,
            b: 0x87 + 0x1234,
            c: 0x87,
            d: 0x1234,
            s: 0x1234,
        },
    }

    sub {
        code: [
            0x82, 0x1d, 103, // sub b, 103
            0x82, 0x21, // sub c, b
            0x82, 0x3e, 0x34, 0x12, // sub d, 0x1234 (4660)
            0x82, 0x10, // sub b, a
            0x82, 0x0d, 58, // sub a, 58
            0x82, 0x73, // sub s, d
            0x82, 0x13, // sub b, d
        ],
        registers: {
            a: 65478,
            b: 4557,
            c: 103,
            d: 60876,
            s: 4660,
        },
    }

    xor {
        code: [
            0x83, 0x1d, 0x6e, // xor b, 0x6e
            0x83, 0x21, // xor c, b
            0x83, 0x3e, 0x34, 0x12, // xor d, 0x1234
            0x83, 0x10, // xor b, a
            0x83, 0x0d, 0x37, // xor a, 0x37
            0x83, 0x73, // xor s, d
            0x83, 0x13, // xor b, d
            0x83, 0x22, // xor c, c
        ],
        registers: {
            a: 0x37,
            b: 0x125a,
            d: 0x1234,
            s: 0x1234,
        },
    }

    and {
        code: [
            0x80, 0x2e, 0x58, 0xf2, // mov c, 0xf258
            0x80, 0x3e, 0x32, 0xc3, // mov d, 0xc332
            0x84, 0x23, // and c, d
            0x80, 0x03, // mov a, d
            0x84, 0x0d, 0xec, // and a, 0xec
        ],
        registers: {
            a: 0x20,
            c: 0xc210,
            d: 0xc332,
        },
    }

    or {
        code: [
            0x80, 0x2e, 0x58, 0xf2, // mov c, 0xf258
            0x80, 0x3e, 0x32, 0xc3, // mov d, 0xc332
            0x85, 0x23, // or c, d
            0x80, 0x03, // mov a, d
            0x85, 0x0d, 0xec, // or a, 0xec
        ],
        registers: {
            a: 0xc3fe,
            c: 0xf37a,
            d: 0xc332,
        },
    }

    shl {
        code: [
            0x80, 0x2e, 0x58, 0xf2, // mov c, 0xf258
            0x80, 0x3e, 0x32, 0xc3, // mov d, 0xc332
            0x80, 0x6b, // mov p, 3
            0x86, 0x0a, // shl a, 2
            0x86, 0x2b, // shl c, 3
            0x86, 0x3d, 7, // shl d, 7
            0x80, 0x12, // mov b, c
            0x86, 0x16, // shl b, p
            
        ],
        registers: {
            b: 0x9600,
            c: 0x92c0,
            d: 0x9900,
            p: 3,
        },
    }

    shr {
        code: [
            0x80, 0x2e, 0x58, 0xf2, // mov c, 0xf258
            0x80, 0x3e, 0x32, 0xc3, // mov d, 0xc332
            0x80, 0x6b, // mov p, 3
            0x87, 0x0a, // shr a, 2
            0x87, 0x2b, // shr c, 3
            0x87, 0x3d, 7, // shr d, 7
            0x80, 0x12, // mov b, c
            0x87, 0x16, // shr b, p
            
        ],
        registers: {
            b: 0x03c9,
            c: 0x1e4b,
            d: 0x0186,
            p: 3,
        },
    }

    stack {
        code: [
            0b1000_0010, 0b0111_1010, // sub s, 2
            0b0100_1001, // push 1
            0b0100_1101, 22, // push 22
            0b0100_1101, 53, // push 53
            0b0011_0010, // pop c
            0b0011_0101, // pop j
        ],
        registers: {
            c: 53,
            j: 22,
            s: 0xfffc,
        },
        memory: {
            0xfffe: [1],
            0xfffc: [22],
            0xfffa: [53],
        },
    }
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
        0b1000_0011, 0b0000_0000, // xor a, a
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

#[test]
fn full_random() {
    let seed = 0xcafe_babe_dead_beef_u64.to_le_bytes();
    let mut random = rand_xoshiro::Xoroshiro64Star::from_seed(seed);
    let mut hashes = Vec::new();

    for bit_pattern in 0..=0xffu8 {
        let mut emulator = random_emulator(bit_pattern);
        let place = random.gen::<u16>();
        emulator.memory_mut()[usize::from(place)] = bit_pattern;
        emulator.instruction_pointer = place;
        emulator.cycle();
        let mut hasher = metrohash::MetroHash::default();
        hash_state(&emulator, &mut hasher);
        hashes.push(hasher.finish());
    }

    let expected_hashes = [
        0xfcb979626895a7f8, 0x6bcf001eb4ad4ddb, 0x4ad20611d5ecd02d, 0xb373d574a42668fc,
        0xa36b7ed01706e890, 0x0a539e0925b3e91d, 0x2393692a85f2aa2d, 0xd2b2f85c368527a9,
        0x33d167a962eef35c, 0x0b85fe8f46037ead, 0x918ecc14d0a3edc3, 0x27d78f8cb005a9ae,
        0xc173689c5177589e, 0xbd95d1b05bd455aa, 0x771488f46ca6c763, 0xc87200d0239b6bdf,
        0xb3741d551878953b, 0x5b7c31503014d0dd, 0x7c3df6505bc5d917, 0xa121c43d63db0002,
        0xf0b293c8aa81ea6a, 0x1b7da34ff4cdb9b1, 0x3f69054c8a7e5c28, 0xe2b2b6ce80d4e24c,
        0x18e363005c868034, 0x43a6273f27f1f64d, 0xd40e325b0114e730, 0xb0770bd35f20d20c,
        0x3f6db86c731d996c, 0x2ffd7cd8d9a9254c, 0x5d8e7f25c3386cc6, 0xb98660c907ba77fc,
        0xcbc318b85660b1da, 0x553e7936b1952023, 0xd20a27e8256a0e06, 0xb3389bc9d087d4db,
        0x18add181099e52ba, 0xa8a985b3cec9794a, 0x95c57fe767ec1caf, 0xe6bf2f4bea77488d,
        0x5aa83aac3ecd8401, 0xcc0a247fbe3da9f3, 0x664664e052105fe6, 0xe2bfc2feb9aeb374,
        0x8252fd01a2c41716, 0xade93c97035e9ee6, 0x86c4a9e7479a9ed5, 0xf6e607d776e3a500,
        0x97d543086a8e38b3, 0x1cc2f5c87ad3d891, 0x11bfd477f92e910f, 0x3001f1b3edc5d498,
        0x009e2767aac5e392, 0x261f03646a83f4e3, 0x13faba0c95c6d3c1, 0x28beaf6c112ad249,
        0x00b0b9b23b67a5f3, 0xbff7e9fbc2fe153e, 0xdabe848f32585fc0, 0x7f5f786a57fe61aa,
        0x35c6fa272e25efcc, 0x4c8716cf95ead28f, 0xb4dc89183ee08a31, 0x6b5063c863875b13,
        0xaceda320c61b1af3, 0x4ce72d48a3997168, 0xaad8f102b210695d, 0x2cf61c06cf777f23,
        0xaba16a74f0c4d44a, 0x10e97ace89ab8fc7, 0x356ef14f40a5463d, 0x6237a68d4adfd3c2,
        0xdf8ff351f99d76e3, 0xfdb4863cd9a0d9a9, 0x7bbb28180898916d, 0xb3f31260cce72c27,
        0x18ae3cd10daf205e, 0x0ab3699ddc92fd10, 0xa59c91213022f766, 0x7e0d8e8144cbbb6f,
        0xfd29edf7f15a1d47, 0x6cef3c4947c2f821, 0xd47e7a11d507cbee, 0x5ab5ef8c495a18de,
        0xfd0f4da7cbfa5692, 0x0a7032fbed94b3dc, 0x87e26dd6c8e327d7, 0xfe3b92c55ff49458,
        0xc16331a372374d1f, 0xfd48493c678bbc66, 0x8cd37e0dbc1f388e, 0x6973f85336dfbe0d,
        0x431317291f2bdd42, 0x5a0b9fb2a942be37, 0x95ca1c96ad436c3e, 0x94bdd9a55a00e50d,
        0x4078ba484f6ffda6, 0x2882a779a094f3d9, 0xc8b5156a1a9b234e, 0xdb6ef274e58097c8,
        0xdb782e599f9c47fa, 0xf989c0bad47984ab, 0x6805f26018ddbd28, 0x8dab0050d90599e7,
        0xbbf3f73e163a7f27, 0xd4b85ee3f4795a77, 0x34b5fcc24bf9c0c1, 0xcce905f8dfde2d1a,
        0x6e48ea639df5563b, 0x8861d64191dcb00b, 0x76c3ed03ba011e29, 0x6e800d52f5659445,
        0x10fa85a7dd56e16e, 0x8edbc9d9c13b9e6a, 0x394df5eb5058588d, 0xee176468a46140ba,
        0xbd674d108b54f579, 0x9d27136ae6b199ef, 0x2c2bc1f6b4847c8f, 0xeda585d0a9d18a0d,
        0xbb08e61664546db4, 0x440ad12e2543bb71, 0x5578644a4a891ab0, 0x00b150667bbd23dc,
        0x66e60798c996c55e, 0x9648f35f7a245bc9, 0x994a3bd29f2248d1, 0x33b825dab0730de9,
        0x58f3db94f3479eda, 0x68c262450a325824, 0x181f6a948fc834e6, 0x8a6d11d34d7c97f2,
        0x87c1b16a5b96aa71, 0x4dd34058a32de4ad, 0x83d4698e6c19faca, 0x066b157d6f9c6d57,
        0x2a4d544d35c05283, 0xb964e42475bee480, 0x43c99d1729342370, 0x21198ad051a9074f,
        0x358011006fa97a48, 0x4402d10941fb33a5, 0x4d678884c9769624, 0xa0f1f497626ed555,
        0x9fd59352ce1f75d0, 0xd2375bc9c514a7fb, 0xb595b892362a0004, 0x49b78b77f0b447c8,
        0xd4714443fcd87707, 0x90af04de978dd1cc, 0xdaf5a16faa00f23b, 0x4eb7e8059409fff4,
        0xf96ec8ef80e25219, 0xdb15b61e5fd109ed, 0xc08b8fb2a50eeac8, 0xa59e63e816a3a262,
        0x61c7be3f2808c676, 0x37c316605ccf51ea, 0x28279952d99c321c, 0x0f13d8609b37d32d,
        0x4d8fca708c7ddf5f, 0x2a90d6f75781a666, 0x26fb3b3820433120, 0x71ff0bf9c0a34a2b,
        0x1415f65b0222fc9e, 0x5b92fc7cd5789c00, 0x0866ee715d2256d7, 0x089e697d3a72779a,
        0x2d45a64209103684, 0xe0a8ddf996cd1751, 0x7d5aaad436176249, 0x9d60dcc112e72041,
        0x43da962acd81612a, 0x312b3eb0372a922b, 0x7cc592a881751153, 0x6ec8c47ce7cccaa9,
        0x47bf3ebf2a132c3a, 0xa07139f837e1382a, 0x4d7ac32c04680ee9, 0x6daafd8e2cf63469,
        0x4ca47c057ab73dea, 0x77d7b89fa76ee6e5, 0x167514b2993a31f7, 0x22d8b4bf532c6dae,
        0xacaf4ad1fbcc59d2, 0x1b5ed537e185e670, 0xc52d675786c76f90, 0x01f5328ee614a7b7,
        0x368448d39331a80d, 0xb63bf64f41358dc7, 0xb8292ed0c08393b9, 0x36462d40d5baf820,
        0xe4e796c5875f41ee, 0x906ee0d0147c01b5, 0x41aab10868f688d8, 0xd4481657cfe9470e,
        0xb9a7f910875c29d7, 0xf89b1a17184eeef4, 0xbe2ac24e77ca893d, 0xbec4d63e9122f1a2,
        0x270892a5d0fc4231, 0x101328fb4e2a9846, 0x59064c153abf6510, 0x2aa8555d65f95dbb,
        0x1beaf4a10e9e6345, 0x8e36a13874356dc2, 0x282b3543e303df61, 0xcaac9a0be71b1018,
        0x940cd0c40cad1bb8, 0xe9944f05d3efe88a, 0xb739a3740d59873e, 0x963a598606c29ea8,
        0x421780ac318c1762, 0x1d557899a7fc5975, 0xa986718890c12e11, 0x5acbb7b0991e41de,
        0xe97e2dfe866da4f1, 0xf3ffa11c401bf10f, 0x6b279492b6696efd, 0x7f83fd77163a10fe,
        0x83ff3bcd3376dbb4, 0xf2e9878ef1b84b01, 0x8ae7fc4c51684358, 0xc5cdbdb7844a8e95,
        0xe8e5faa27621d6c0, 0x2ddb2c5834b37cab, 0x01e9d745c4d8dca2, 0xf47a091783701a83,
        0x549d15cd80f4d1db, 0xbf38028f4b23319a, 0x08759cc3349a105a, 0xe0a5706efe5074f9,
        0xd30b58f69c92c688, 0x9b95cb6440b2182e, 0x094c4545083dd8fb, 0xf96d776b9a2c6ed8,
        0xb5f2ef48e0971245, 0xe616b65118f83269, 0x3deb495f300e843d, 0xa44770b710735ea2,
        0xa223e2f4bf1ffa75, 0x2a94d9744ac7fea3, 0xeb6b6def1ab2c408, 0x21623bf4cef258d1,
        0x730080dab3883c99, 0x3a8e66d68cc5cec2, 0x56c3391ea9e7804c, 0x199efc47458d12df,
        0x5bcce1012a11c652, 0x6f623165ff5223cf, 0x438b5e7fa297c098, 0x66b0a920beb81398,
        0xefa04a04d55f8709, 0x7db95736aadc8b51, 0xe90a44554ea7b117, 0xc95509f5c9c32326,
    ];

    for (b, (&actual, &expected)) in hashes.iter().zip(expected_hashes.iter()).enumerate() {
        assert_eq!(expected, actual, "state hash from executing 0x{:>02x}", b);
    }
}

fn random_emulator(seed: u8) -> Emulator<Memory, DiskMemory, impl Tracer> {
    let seed = u64::from(seed).to_le_bytes();
    let mut random = rand_xoshiro::Xoroshiro64Star::from_seed(seed);
    let mut emulator = emulator();
    for byte in &mut emulator.memory_mut()[..] {
        *byte = random.gen();
    }
    emulator.registers.a = random.gen();
    emulator.registers.b = random.gen();
    emulator.registers.c = random.gen();
    emulator.registers.d = random.gen();
    emulator.registers.i = random.gen();
    emulator.registers.j = random.gen();
    emulator.registers.p = random.gen();
    emulator.registers.s = random.gen();
    emulator.instruction_pointer = random.gen();
    emulator
}

fn hash_state(
    emulator: &Emulator<Memory, DiskMemory, impl Tracer>,
    hasher: &mut impl Hasher,
) {
    hasher.write_u16(emulator.registers.a);
    hasher.write_u16(emulator.registers.b);
    hasher.write_u16(emulator.registers.c);
    hasher.write_u16(emulator.registers.d);
    hasher.write_u16(emulator.registers.i);
    hasher.write_u16(emulator.registers.j);
    hasher.write_u16(emulator.registers.p);
    hasher.write_u16(emulator.registers.s);
    hasher.write_u16(emulator.instruction_pointer);
    for &byte in &emulator.memory()[..] {
        hasher.write_u8(byte);
    }
    hasher.write_u8(match emulator.state {
        CpuState::Halted => 0,
        CpuState::Running => 1,
        CpuState::Waiting => 2,
    });
}
