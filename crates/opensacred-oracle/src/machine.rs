//! The emulated machine (docs/04 §2.3): a 32-bit x86 CPU of the Unicorn emulator with the image
//! of `Sacred.exe` mapped at its base, its sections at their virtual addresses (`.data` and
//! `.rdata` from the file, so that the compiled-in tables are there), a stack and a scratch area
//! of the oracle's own, and every import's slot pointing at a thunk of its own.
//!
//! A call runs the original's machine code from a trampoline of the oracle's own that resets the
//! FPU to the control word chosen, calls the function with its arguments on the stack (and `this`
//! in `ECX`), and keeps what it returns: `EAX`, and `ST0` stored as a double for a function that
//! returns a float. A function the code calls that the oracle replaces is a stub: it returns a
//! value and removes its arguments, as the function would. A call into an import that has no
//! stub, an access to memory that is not mapped, an invalid instruction or an interrupt stops the
//! call with an error that names it and the address: the oracle never guesses.

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use unicorn_engine::unicorn_const::{Arch, HookType, MemType, Mode, Prot, SECOND_SCALE};
use unicorn_engine::{RegisterX86, Unicorn};

use crate::pe::Image;

/// The stack: 1 MiB below the image (which `Sacred.exe` lays out from 0x400000).
const STACK: u32 = 0x0010_0000;
const STACK_SIZE: u32 = 0x0010_0000;
/// The scratch area: the trampolines, the FPU's control word, the results and the objects that
/// the inputs make (a function's `this`).
const SCRATCH: u32 = 0x6000_0000;
const SCRATCH_SIZE: u32 = 0x0100_0000;
/// Where the trampolines' FPU control word and the stored `ST0` live in the scratch area.
const CONTROL_WORD: u32 = SCRATCH;
const RESULT: u32 = SCRATCH + 8;
/// The trampolines, from here on.
const TRAMPOLINES: u32 = SCRATCH + 0x1000;
/// The objects, from here on.
const OBJECTS: u32 = SCRATCH + 0x10_0000;
/// The imports' thunks, 16 bytes each.
const THUNKS: u32 = 0x7000_0000;
const THUNK_SIZE: u32 = 16;
/// A call's limits: instructions and time.
const MOST_INSTRUCTIONS: usize = 200_000_000;
const MOST_SECONDS: u64 = 30;

/// A function the oracle replaces: what it returns in `EAX`, and how many bytes of arguments it
/// removes from the stack as it returns (`stdcall` and `thiscall`; 0 for `cdecl`).
#[derive(Clone, Debug)]
pub struct Stub {
    pub name: String,
    pub value: u32,
    pub pops: u32,
}

/// What a call returned.
#[derive(Clone, Copy, Debug)]
pub struct Returned {
    pub eax: u32,
    /// `ST0` stored as a double, for a function that returns a float.
    pub st0: Option<f64>,
}

/// The machine's state that the hooks reach.
#[derive(Default)]
struct State {
    /// The imports' names (`DLL!function`) by their thunks.
    imports: HashMap<u32, String>,
    /// The stubs by the addresses they replace (a thunk or a function of the image).
    stubs: HashMap<u32, Stub>,
    /// What stopped the last call, when something did.
    fault: Option<String>,
}

pub struct Machine<'a> {
    uc: Unicorn<'a, State>,
    /// The trampolines made, by the function they call and whether it returns a float.
    trampolines: HashMap<(u32, bool), (u32, u32)>,
    next_trampoline: u32,
    next_object: u32,
    /// Where the objects that `clear_objects` forgets begin: those before are kept.
    kept_objects: u32,
    /// The imports' thunks by `DLL!function`, the DLL's name in lower case.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no set's function calls an import yet")
    )]
    thunks: HashMap<String, u32>,
}

/// The FPU's control word with every exception masked, rounding to nearest, and the precision of
/// `bits` of mantissa: 24 (single), 53 (double, the runtime's default `0x27F`) or 64 (extended).
pub fn control_word(bits: u8) -> Result<u16> {
    let precision = match bits {
        24 => 0,
        53 => 2,
        64 => 3,
        _ => bail!("a precision of {bits} bits: the FPU has 24, 53 and 64"),
    };
    Ok(0x007F | (precision << 8))
}

impl<'a> Machine<'a> {
    /// A machine with `image` mapped and the FPU's control word `control` for every call.
    pub fn new(image: &Image, control: u16) -> Result<Self> {
        let mut uc = Unicorn::new_with_data(Arch::X86, Mode::MODE_32, State::default())
            .context("starting the emulator")?;
        let size = u32::try_from(image.memory.len())?.next_multiple_of(0x1000);
        uc.mem_map(u64::from(image.base), u64::from(size), Prot::ALL)
            .context("mapping the image")?;
        uc.mem_write(u64::from(image.base), &image.memory)
            .context("writing the image")?;
        uc.mem_map(
            u64::from(STACK),
            u64::from(STACK_SIZE),
            Prot::READ | Prot::WRITE,
        )
        .context("mapping the stack")?;
        uc.mem_map(u64::from(SCRATCH), u64::from(SCRATCH_SIZE), Prot::ALL)
            .context("mapping the scratch area")?;
        uc.mem_write(u64::from(CONTROL_WORD), &control.to_le_bytes())?;

        // Each import's slot points at a thunk of its own, an `int3` that never runs: the hook
        // below takes the call there first.
        let count = u32::try_from(image.imports.len())?;
        let thunks_size = (count * THUNK_SIZE).max(1).next_multiple_of(0x1000);
        uc.mem_map(u64::from(THUNKS), u64::from(thunks_size), Prot::ALL)
            .context("mapping the thunks")?;
        uc.mem_write(
            u64::from(THUNKS),
            &vec![0xCC; usize::try_from(thunks_size)?],
        )?;
        let mut thunks = HashMap::new();
        for (i, import) in (0_u32..).zip(&image.imports) {
            let thunk = THUNKS + i * THUNK_SIZE;
            uc.mem_write(u64::from(image.base + import.slot), &thunk.to_le_bytes())
                .with_context(|| format!("the slot of {}!{}", import.dll, import.name))?;
            let name = format!("{}!{}", import.dll.to_ascii_lowercase(), import.name);
            uc.get_data_mut().imports.insert(thunk, name.clone());
            thunks.insert(name, thunk);
        }
        uc.add_code_hook(
            u64::from(THUNKS),
            u64::from(THUNKS + thunks_size - 1),
            |uc, address, _| run_stub(uc, address),
        )
        .context("hooking the thunks")?;

        uc.add_mem_hook(
            HookType::MEM_READ_UNMAPPED
                | HookType::MEM_WRITE_UNMAPPED
                | HookType::MEM_FETCH_UNMAPPED
                | HookType::MEM_WRITE_PROT
                | HookType::MEM_FETCH_PROT,
            1,
            0,
            |uc, kind, address, size, _| {
                let at = uc.reg_read(RegisterX86::EIP).unwrap_or(0);
                let what = match kind {
                    MemType::READ_UNMAPPED => "a read of unmapped memory",
                    MemType::WRITE_UNMAPPED => "a write to unmapped memory",
                    MemType::FETCH_UNMAPPED => "a jump to unmapped memory",
                    MemType::WRITE_PROT => "a write to read-only memory",
                    _ => "a jump to memory that is not code",
                };
                fault(
                    uc,
                    format!("{what}: {size} bytes at {address:#x}, at {at:#x}"),
                );
                false
            },
        )
        .context("hooking the memory")?;
        uc.add_insn_invalid_hook(|uc| {
            let at = uc.reg_read(RegisterX86::EIP).unwrap_or(0);
            fault(uc, format!("an invalid instruction at {at:#x}"));
            false
        })
        .context("hooking invalid instructions")?;
        uc.add_intr_hook(|uc, number| {
            let at = uc.reg_read(RegisterX86::EIP).unwrap_or(0);
            fault(uc, format!("the interrupt {number} at {at:#x}"));
        })
        .context("hooking the interrupts")?;

        Ok(Self {
            uc,
            trampolines: HashMap::new(),
            next_trampoline: TRAMPOLINES,
            next_object: OBJECTS,
            kept_objects: OBJECTS,
            thunks,
        })
    }

    /// Replaces the import `dll!function` by a stub that returns `value` and removes `pops`
    /// bytes of arguments.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no set's function calls an import yet")
    )]
    pub fn stub_import(&mut self, dll: &str, function: &str, value: u32, pops: u32) -> Result<()> {
        let name = format!("{}!{function}", dll.to_ascii_lowercase());
        let Some(&thunk) = self.thunks.get(&name) else {
            bail!("the image imports no {dll}!{function}");
        };
        self.uc
            .get_data_mut()
            .stubs
            .insert(thunk, Stub { name, value, pops });
        Ok(())
    }

    /// Changes the value that the stub at `address` returns.
    pub fn restub(&mut self, address: u32, value: u32) -> Result<()> {
        match self.uc.get_data_mut().stubs.get_mut(&address) {
            Some(stub) => {
                stub.value = value;
                Ok(())
            }
            None => bail!("no stub at {address:#x}"),
        }
    }

    /// Replaces the image's function at `address` by `stub`.
    pub fn stub_function(&mut self, address: u32, stub: Stub) -> Result<()> {
        let name = stub.name.clone();
        self.uc.get_data_mut().stubs.insert(address, stub);
        self.uc
            .add_code_hook(u64::from(address), u64::from(address), |uc, at, _| {
                run_stub(uc, at);
            })
            .with_context(|| format!("hooking {name} at {address:#x}"))?;
        Ok(())
    }

    /// Writes `bytes` at `address`.
    pub fn write(&mut self, address: u32, bytes: &[u8]) -> Result<()> {
        self.uc
            .mem_write(u64::from(address), bytes)
            .with_context(|| format!("writing {} bytes at {address:#x}", bytes.len()))
    }

    /// Reads `size` bytes at `address`.
    pub fn read(&self, address: u32, size: usize) -> Result<Vec<u8>> {
        self.uc
            .mem_read_as_vec(u64::from(address), size)
            .with_context(|| format!("reading {size} bytes at {address:#x}"))
    }

    /// A new object of `size` bytes of zeros in the scratch area, aligned to 16.
    pub fn object(&mut self, size: u32) -> Result<u32> {
        let address = self.next_object;
        let end = address + size.next_multiple_of(16);
        if end > SCRATCH + SCRATCH_SIZE {
            bail!("the scratch area is full");
        }
        self.write(address, &vec![0; usize::try_from(size)?])?;
        self.next_object = end;
        Ok(address)
    }

    /// Forgets the objects made since the last `keep_objects`, so that the next ones take their
    /// place.
    pub fn clear_objects(&mut self) {
        self.next_object = self.kept_objects;
    }

    /// Keeps the objects made so far from `clear_objects`.
    pub fn keep_objects(&mut self) {
        self.kept_objects = self.next_object;
    }

    /// Calls the function at `function` with `this` in `ECX` and the double words `args` on the
    /// stack, the first at the lowest address; `float` when it returns a float in `ST0`.
    pub fn call(
        &mut self,
        function: u32,
        this: Option<u32>,
        args: &[u32],
        float: bool,
    ) -> Result<Returned> {
        let (start, end) = self.trampoline(function, float)?;
        let top = STACK + STACK_SIZE - 0x100;
        let esp = top - 4 * u32::try_from(args.len())?;
        let stack: Vec<u8> = args.iter().flat_map(|arg| arg.to_le_bytes()).collect();
        self.write(esp, &stack)?;
        for register in [
            RegisterX86::EAX,
            RegisterX86::EBX,
            RegisterX86::EDX,
            RegisterX86::ESI,
            RegisterX86::EDI,
            RegisterX86::EBP,
        ] {
            self.uc.reg_write(register, 0)?;
        }
        self.uc
            .reg_write(RegisterX86::ECX, u64::from(this.unwrap_or(0)))?;
        self.uc.reg_write(RegisterX86::ESP, u64::from(esp))?;
        self.uc.get_data_mut().fault = None;
        let run = self.uc.emu_start(
            u64::from(start),
            u64::from(end),
            MOST_SECONDS * SECOND_SCALE,
            MOST_INSTRUCTIONS,
        );
        if let Some(fault) = self.uc.get_data_mut().fault.take() {
            bail!("calling {function:#x}: {fault}");
        }
        run.with_context(|| format!("calling {function:#x}"))?;
        let at = self.uc.reg_read(RegisterX86::EIP)?;
        if at != u64::from(end) {
            bail!(
                "calling {function:#x}: stopped at {at:#x} before it returned \
                 (more than {MOST_INSTRUCTIONS} instructions or {MOST_SECONDS} s)"
            );
        }
        let eax = u32::try_from(self.uc.reg_read(RegisterX86::EAX)?)?;
        let st0 = if float {
            let bytes: [u8; 8] = self.read(RESULT, 8)?.try_into().unwrap_or_default();
            Some(f64::from_le_bytes(bytes))
        } else {
            None
        };
        Ok(Returned { eax, st0 })
    }

    /// The trampoline that calls `function`, made once: `fninit`, `fldcw` of the control word,
    /// `call`, and for a float `fstp qword` of `ST0` to the result; it ends where the emulation
    /// stops.
    fn trampoline(&mut self, function: u32, float: bool) -> Result<(u32, u32)> {
        if let Some(&made) = self.trampolines.get(&(function, float)) {
            return Ok(made);
        }
        let start = self.next_trampoline;
        let mut code = vec![0xDB, 0xE3, 0xD9, 0x2D];
        code.extend(CONTROL_WORD.to_le_bytes());
        let after_call = start + u32::try_from(code.len())? + 5;
        code.push(0xE8);
        code.extend(function.wrapping_sub(after_call).to_le_bytes());
        if float {
            code.extend([0xDD, 0x1D]);
            code.extend(RESULT.to_le_bytes());
        }
        let end = start + u32::try_from(code.len())?;
        // A `hlt` where the emulation stops, should it ever run.
        code.push(0xF4);
        self.write(start, &code)?;
        self.next_trampoline = (end + 1).next_multiple_of(16);
        self.trampolines.insert((function, float), (start, end));
        Ok((start, end))
    }
}

/// Records what stopped the call and stops it.
fn fault(uc: &mut Unicorn<'_, State>, what: String) {
    let state = uc.get_data_mut();
    if state.fault.is_none() {
        state.fault = Some(what);
    }
    // The stop takes effect at the end of the block; the fault is reported either way.
    let _ = uc.emu_stop();
}

/// Runs the stub at `address` in place of the code there: its value in `EAX`, the return address
/// popped with its arguments, the code going on there; a thunk without a stub is a fault.
fn run_stub(uc: &mut Unicorn<'_, State>, address: u64) {
    let Ok(address) = u32::try_from(address) else {
        return;
    };
    let stub = uc.get_data().stubs.get(&address).cloned();
    let Some(stub) = stub else {
        if let Some(name) = uc.get_data().imports.get(&address).cloned() {
            let back = return_address(uc).unwrap_or(0);
            fault(
                uc,
                format!("the import {name}, which has no stub, called from {back:#x}"),
            );
        }
        return;
    };
    let result = (|| -> Result<()> {
        let esp = u32::try_from(uc.reg_read(RegisterX86::ESP)?)?;
        let back = return_address(uc)?;
        uc.reg_write(RegisterX86::EAX, u64::from(stub.value))?;
        uc.reg_write(RegisterX86::ESP, u64::from(esp + 4 + stub.pops))?;
        uc.reg_write(RegisterX86::EIP, u64::from(back))?;
        Ok(())
    })();
    if let Err(error) = result {
        fault(uc, format!("the stub {}: {error:#}", stub.name));
    }
}

/// The return address on the top of the stack.
fn return_address(uc: &Unicorn<'_, State>) -> Result<u32> {
    let esp = uc.reg_read(RegisterX86::ESP)?;
    let bytes: [u8; 4] = uc.mem_read_as_vec(esp, 4)?.try_into().unwrap_or_default();
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::tests::synthetic;

    /// The synthetic image's `.text` at 0x401000:
    /// - 0x401000: `int add(int a, int b)`, `cdecl`;
    /// - 0x401010: `int ticks(void)`, which calls `GetTickCount` through its slot and adds 1;
    /// - 0x401020: `float half(float x)`, `cdecl`, returned in `ST0`;
    /// - 0x401030: `int field(this)`, `thiscall`, the word at `this + 4`;
    /// - 0x401040: a read of 0x10 (unmapped);
    /// - 0x401050: `int twice(int a)`, which calls 0x401060 with `a` and doubles what it returns;
    /// - 0x401060: a function that the test replaces.
    fn image() -> Image {
        let mut code = vec![0x90; 0x80];
        let mut put = |at: usize, bytes: &[u8]| code[at..at + bytes.len()].copy_from_slice(bytes);
        // mov eax, [esp+4]; add eax, [esp+8]; ret
        put(
            0x00,
            &[0x8B, 0x44, 0x24, 0x04, 0x03, 0x44, 0x24, 0x08, 0xC3],
        );
        // call [0x402100]; inc eax; ret
        put(0x10, &[0xFF, 0x15, 0x00, 0x21, 0x40, 0x00, 0x40, 0xC3]);
        // fld dword [esp+4]; fmul dword [0x401070]; ret
        put(
            0x20,
            &[
                0xD9, 0x44, 0x24, 0x04, 0xD8, 0x0D, 0x70, 0x10, 0x40, 0x00, 0xC3,
            ],
        );
        // mov eax, [ecx+4]; ret
        put(0x30, &[0x8B, 0x41, 0x04, 0xC3]);
        // mov eax, [0x10]; ret
        put(0x40, &[0xA1, 0x10, 0x00, 0x00, 0x00, 0xC3]);
        // push dword [esp+4]; call 0x401060; add esp, 4; add eax, eax; ret
        put(
            0x50,
            &[
                0xFF, 0x74, 0x24, 0x04, 0xE8, 0x07, 0x00, 0x00, 0x00, 0x83, 0xC4, 0x04, 0x01, 0xC0,
                0xC3,
            ],
        );
        // int3
        put(0x60, &[0xCC]);
        put(0x70, &0.5_f32.to_le_bytes());
        Image::parse(&synthetic(&code)).unwrap()
    }

    #[test]
    fn a_function_of_the_image_runs_on_its_arguments() {
        let mut machine = Machine::new(&image(), control_word(53).unwrap()).unwrap();
        assert_eq!(
            machine.call(0x40_1000, None, &[40, 2], false).unwrap().eax,
            42
        );
        let half = machine
            .call(0x40_1020, None, &[3.0_f32.to_bits()], true)
            .unwrap();
        assert_eq!(half.st0, Some(1.5));
        let this = machine.object(16).unwrap();
        machine.write(this + 4, &7_u32.to_le_bytes()).unwrap();
        assert_eq!(
            machine.call(0x40_1030, Some(this), &[], false).unwrap().eax,
            7
        );
    }

    #[test]
    fn an_import_runs_its_stub_and_one_without_is_named() {
        let mut machine = Machine::new(&image(), control_word(53).unwrap()).unwrap();
        let error = machine.call(0x40_1010, None, &[], false).unwrap_err();
        assert!(
            format!("{error:#}").contains("kernel32.dll!GetTickCount, which has no stub"),
            "{error:#}"
        );
        machine
            .stub_import("KERNEL32.dll", "GetTickCount", 1000, 0)
            .unwrap();
        assert_eq!(machine.call(0x40_1010, None, &[], false).unwrap().eax, 1001);
    }

    #[test]
    fn a_function_of_the_image_can_be_replaced() {
        let mut machine = Machine::new(&image(), control_word(53).unwrap()).unwrap();
        machine
            .stub_function(
                0x40_1060,
                Stub {
                    name: "five".to_owned(),
                    value: 5,
                    pops: 0,
                },
            )
            .unwrap();
        assert_eq!(machine.call(0x40_1050, None, &[9], false).unwrap().eax, 10);
    }

    #[test]
    fn a_read_of_unmapped_memory_stops_the_call() {
        let mut machine = Machine::new(&image(), control_word(53).unwrap()).unwrap();
        let error = machine.call(0x40_1040, None, &[], false).unwrap_err();
        assert!(
            format!("{error:#}").contains("a read of unmapped memory: 4 bytes at 0x10"),
            "{error:#}"
        );
        // The machine goes on after a fault.
        assert_eq!(
            machine.call(0x40_1000, None, &[1, 2], false).unwrap().eax,
            3
        );
    }

    #[test]
    fn the_control_word_takes_the_precision() {
        assert_eq!(control_word(53).unwrap(), 0x27F);
        assert_eq!(control_word(24).unwrap(), 0x07F);
        assert_eq!(control_word(64).unwrap(), 0x37F);
        assert!(control_word(32).is_err());
    }
}
