// Classes from the MSVC RTTI of a 32-bit program whose RTTI analyzer marked the type descriptors
// alone: each complete object locator (signature 0, its type descriptor at +12, its hierarchy
// descriptor at +16) found in the read-only data, each vftable after a pointer to a locator; a
// class namespace for each type descriptor's namespace, its vftables labeled, a function made at
// each slot the analysis left without one, the virtual functions that only its vftables hold and
// the functions that write only its vftables (its constructors and destructors) moved into it.
// Prints the counts. docs/03 §4.1.
//@category OpenSacred

import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.GhidraClass;
import ghidra.program.model.mem.Memory;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.*;
import ghidra.app.util.NamespaceUtils;

public class RttiClassesScript extends GhidraScript {

	private Memory memory;

	private boolean inBlock(long value, MemoryBlock block) {
		return block != null && value >= block.getStart().getOffset() && value <= block.getEnd().getOffset();
	}

	@Override
	public void run() throws Exception {
		memory = currentProgram.getMemory();
		SymbolTable symbols = currentProgram.getSymbolTable();
		MemoryBlock text = memory.getBlock(".text");
		MemoryBlock rdata = memory.getBlock(".rdata");
		// The type descriptors, by address, and their namespaces.
		Map<Long, Namespace> types = new HashMap<>();
		SymbolIterator it = symbols.getAllSymbols(false);
		while (it.hasNext()) {
			Symbol symbol = it.next();
			if (symbol.getName().equals("RTTI_Type_Descriptor")) {
				types.put(symbol.getAddress().getOffset(), symbol.getParentNamespace());
			}
		}
		// The complete object locators in the read-only data.
		Map<Long, Long> locators = new HashMap<>();
		byte[] data = new byte[(int) rdata.getSize()];
		rdata.getBytes(rdata.getStart(), data);
		long base = rdata.getStart().getOffset();
		for (int at = 0; at + 20 <= data.length; at += 4) {
			long type = word(data, at + 12);
			if (word(data, at) == 0 && types.containsKey(type) && inBlock(word(data, at + 16), rdata)) {
				locators.put(base + at, type);
			}
		}
		// The vftables: the word after a pointer to a locator, the code pointers that follow.
		Map<Long, List<Long>> vftables = new TreeMap<>();
		Map<Long, Long> vftableType = new HashMap<>();
		for (int at = 0; at + 8 <= data.length; at += 4) {
			long pointer = word(data, at);
			if (!locators.containsKey(pointer)) {
				continue;
			}
			List<Long> slots = new ArrayList<>();
			for (int slot = at + 4; slot + 4 <= data.length; slot += 4) {
				// Up to the first word that is no pointer into the code: the next vftable's locator
				// pointer, or other data.
				long target = word(data, slot);
				if (!inBlock(target, text)) {
					break;
				}
				slots.add(target);
			}
			if (!slots.isEmpty()) {
				vftables.put(base + at + 4, slots);
				vftableType.put(base + at + 4, locators.get(pointer));
			}
		}
		// The classes, the vftables' labels, and which classes' vftables hold each function.
		Map<Long, GhidraClass> classes = new HashMap<>();
		Map<Long, Set<Long>> holders = new HashMap<>();
		int labeled = 0;
		for (Map.Entry<Long, List<Long>> entry : vftables.entrySet()) {
			long type = vftableType.get(entry.getKey());
			Namespace namespace = types.get(type);
			if (namespace == null || namespace.isGlobal()) {
				continue;
			}
			GhidraClass cls = classes.get(type);
			if (cls == null) {
				cls = namespace instanceof GhidraClass ? (GhidraClass) namespace
						: NamespaceUtils.convertNamespaceToClass(namespace);
				classes.put(type, cls);
			}
			Address vftable = toAddr(entry.getKey());
			if (symbols.getSymbol("vftable", vftable, cls) == null) {
				symbols.createLabel(vftable, "vftable", cls, SourceType.ANALYSIS);
				labeled++;
			}
			for (long target : entry.getValue()) {
				holders.computeIfAbsent(target, k -> new HashSet<>()).add(type);
			}
		}
		// The vftables' slots that the analysis left without a function get one.
		int created = 0;
		for (long target : holders.keySet()) {
			Address entry = toAddr(target);
			if (getFunctionAt(entry) == null && getFunctionContaining(entry) == null) {
				if (getInstructionAt(entry) == null) {
					disassemble(entry);
				}
				if (createFunction(entry, null) != null) {
					created++;
				}
			}
		}
		// Virtual functions held by one class's vftables alone.
		int moved = 0, shared = 0;
		for (Map.Entry<Long, Set<Long>> entry : holders.entrySet()) {
			Function function = getFunctionAt(toAddr(entry.getKey()));
			if (function == null || !function.getParentNamespace().isGlobal()) {
				continue;
			}
			if (entry.getValue().size() == 1) {
				function.setParentNamespace(classes.get(entry.getValue().iterator().next()));
				moved++;
			}
			else {
				shared++;
			}
		}
		// The functions that write the vftables of one class alone: its constructors and destructors.
		Map<Function, Set<Long>> writers = new HashMap<>();
		ReferenceManager references = currentProgram.getReferenceManager();
		for (long vftable : vftables.keySet()) {
			for (Reference reference : references.getReferencesTo(toAddr(vftable))) {
				Function function = getFunctionContaining(reference.getFromAddress());
				if (function != null) {
					writers.computeIfAbsent(function, k -> new HashSet<>()).add(vftableType.get(vftable));
				}
			}
		}
		int structors = 0;
		for (Map.Entry<Function, Set<Long>> entry : writers.entrySet()) {
			Function function = entry.getKey();
			if (entry.getValue().size() == 1 && function.getParentNamespace().isGlobal()
					&& classes.containsKey(entry.getValue().iterator().next())) {
				function.setParentNamespace(classes.get(entry.getValue().iterator().next()));
				structors++;
			}
		}
		println(String.format(
			"rtti classes: %d type descriptors, %d locators, %d vftables (%d labeled), %d classes, %d functions created at the vftables' slots, %d virtual functions moved (%d shared by several classes), %d constructors or destructors moved",
			types.size(), locators.size(), vftables.size(), labeled, classes.size(), created, moved, shared, structors));
	}

	private static long word(byte[] data, int at) {
		return (data[at] & 0xFFL) | (data[at + 1] & 0xFFL) << 8 | (data[at + 2] & 0xFFL) << 16
			| (data[at + 3] & 0xFFL) << 24;
	}
}
