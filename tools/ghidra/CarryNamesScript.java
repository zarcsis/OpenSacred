// Carries names and classes into this program from another program of the project (its path the
// argument) over the functions that match by the MD5 of their normalized bytes (docs/01 §5.1:
// the rel32 of call, jmp and jcc and every 32-bit word that points into the image zeroed),
// 32 bytes or more, unique on both sides: a function of a default name in the global namespace
// takes the other's name, if not a default one, and its namespace, created as a class where the
// other's is one. Prints the counts.
//@category OpenSacred

import java.security.MessageDigest;
import java.util.*;

import ghidra.app.script.GhidraScript;
import ghidra.app.util.NamespaceUtils;
import ghidra.framework.model.DomainFile;
import ghidra.program.model.listing.*;
import ghidra.program.model.symbol.*;

public class CarryNamesScript extends GhidraScript {
	private static final long IMAGE_START = 0x400000L;
	private static final long IMAGE_END = 0x1900000L;

	@Override
	public void run() throws Exception {
		String otherPath = getScriptArgs()[0];
		DomainFile file = state.getProject().getProjectData().getFile(otherPath);
		Program other = (Program) file.getReadOnlyDomainObject(this, DomainFile.DEFAULT_VERSION, monitor);
		try {
			Map<String, List<Function>> mine = hashes(currentProgram);
			Map<String, List<Function>> theirs = hashes(other);
			int matched = 0, named = 0, classed = 0;
			for (Map.Entry<String, List<Function>> entry : mine.entrySet()) {
				List<Function> others = theirs.get(entry.getKey());
				if (entry.getValue().size() != 1 || others == null || others.size() != 1) {
					continue;
				}
				matched++;
				Function target = entry.getValue().get(0);
				Function source = others.get(0);
				if (target.getSymbol().getSource() != SourceType.DEFAULT
						|| !target.getParentNamespace().isGlobal()) {
					continue;
				}
				Namespace sourceSpace = source.getParentNamespace();
				boolean defaultName = source.getSymbol().getSource() == SourceType.DEFAULT;
				if (defaultName && sourceSpace.isGlobal()) {
					continue;
				}
				if (!sourceSpace.isGlobal()) {
					Namespace space = NamespaceUtils.createNamespaceHierarchy(
						sourceSpace.getName(true), null, currentProgram, SourceType.IMPORTED);
					if (sourceSpace.getSymbol().getSymbolType() == SymbolType.CLASS
							&& !(space instanceof GhidraClass)) {
						space = NamespaceUtils.convertNamespaceToClass(space);
					}
					target.setParentNamespace(space);
					classed++;
				}
				if (!defaultName) {
					try {
						target.setName(source.getName(), SourceType.IMPORTED);
						named++;
					}
					catch (Exception e) {
						println("not named " + target.getEntryPoint() + " " + source.getName() + ": " + e.getMessage());
					}
				}
			}
			println(String.format("carried from %s: %d functions matched uniquely, %d named, %d moved into namespaces",
				otherPath, matched, named, classed));
		}
		finally {
			other.release(this);
		}
	}

	private Map<String, List<Function>> hashes(Program program) throws Exception {
		Map<String, List<Function>> out = new HashMap<>();
		Listing listing = program.getListing();
		for (Function function : program.getFunctionManager().getFunctions(true)) {
			if (function.isThunk() || function.isExternal()) {
				continue;
			}
			MessageDigest md5 = MessageDigest.getInstance("MD5");
			int size = 0;
			for (Instruction instruction : listing.getInstructions(function.getBody(), true)) {
				byte[] bytes = instruction.getBytes();
				normalize(bytes);
				md5.update(bytes);
				size += bytes.length;
			}
			if (size < 32) {
				continue;
			}
			String hash = HexFormat.of().formatHex(md5.digest());
			out.computeIfAbsent(hash, k -> new ArrayList<>()).add(function);
		}
		return out;
	}

	private static void normalize(byte[] b) {
		int n = b.length;
		// call rel32, jmp rel32, jcc rel32.
		if (n == 5 && ((b[0] & 0xFF) == 0xE8 || (b[0] & 0xFF) == 0xE9)) {
			zero(b, 1);
		}
		if (n == 6 && (b[0] & 0xFF) == 0x0F && (b[1] & 0xF0) == 0x80) {
			zero(b, 2);
		}
		for (int i = 0; i + 4 <= n; i++) {
			long value = (b[i] & 0xFFL) | (b[i + 1] & 0xFFL) << 8 | (b[i + 2] & 0xFFL) << 16
				| (b[i + 3] & 0xFFL) << 24;
			if (value >= IMAGE_START && value < IMAGE_END) {
				zero(b, i);
			}
		}
	}

	private static void zero(byte[] b, int at) {
		for (int i = at; i < at + 4 && i < b.length; i++) {
			b[i] = 0;
		}
	}
}
