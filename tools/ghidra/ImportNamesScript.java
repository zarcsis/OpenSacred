// Names the functions and labels the data of a file of lines "hex address<TAB>name" (# comments):
// a function at an address takes the name unless it has it already (an analyzer's, a thunk's
// target's) or a user or an import named it otherwise; another address in memory gets a label.
// Prints the counts. The names file of tools/ghidra/community_names.py; docs/03 §4.1.
//@category OpenSacred

import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.List;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.SymbolTable;

public class ImportNamesScript extends GhidraScript {
	@Override
	public void run() throws Exception {
		String[] args = getScriptArgs();
		List<String> lines = Files.readAllLines(Paths.get(args[0]));
		SymbolTable symbols = currentProgram.getSymbolTable();
		int functions = 0, labels = 0, kept = 0, missing = 0, failed = 0;
		for (String line : lines) {
			if (line.isBlank() || line.startsWith("#")) {
				continue;
			}
			String[] fields = line.split("\t");
			Address address = toAddr(Long.parseLong(fields[0].trim(), 16));
			String name = fields[1].trim();
			try {
				Function function = getFunctionAt(address);
				if (function != null) {
					SourceType source = function.getSymbol().getSource();
					// A thunk shows its target's name; an analyzer may have found the name already.
					if (function.getName().equals(name)) {
						kept++;
					}
					else if (source == SourceType.DEFAULT || source == SourceType.ANALYSIS) {
						function.setName(name, SourceType.IMPORTED);
						functions++;
					}
					else {
						kept++;
					}
				}
				else if (currentProgram.getMemory().contains(address)) {
					symbols.createLabel(address, name, SourceType.IMPORTED);
					labels++;
				}
				else {
					missing++;
				}
			}
			catch (Exception e) {
				failed++;
				println("failed " + fields[0] + " " + name + ": " + e.getMessage());
			}
		}
		println(String.format("names: %d functions named, %d labels, %d kept, %d outside memory, %d failed",
			functions, labels, kept, missing, failed));
	}
}
