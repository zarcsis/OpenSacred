// Prints how many symbols name the MSVC RTTI structures and vftables, and the analysis options
// whose names mention RTTI or Class, with whether they ran.
//@category OpenSacred

import java.util.Map;
import java.util.TreeMap;

import ghidra.app.script.GhidraScript;
import ghidra.framework.options.Options;
import ghidra.program.model.listing.Program;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class RttiStatsScript extends GhidraScript {
	@Override
	public void run() throws Exception {
		Map<String, Integer> counts = new TreeMap<>();
		String[] kinds = { "RTTI_Type_Descriptor", "RTTI_Complete_Object_Locator",
			"RTTI_Class_Hierarchy_Descriptor", "RTTI_Base_Class_Descriptor", "RTTI_Base_Class_Array",
			"vftable", "meta" };
		SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(false);
		while (it.hasNext()) {
			Symbol symbol = it.next();
			for (String kind : kinds) {
				if (symbol.getName().contains(kind)) {
					counts.merge(kind, 1, Integer::sum);
				}
			}
		}
		println("rtti symbols: " + counts);
		Options analysis = currentProgram.getOptions(Program.ANALYSIS_PROPERTIES);
		for (String name : analysis.getOptionNames()) {
			if (name.contains("RTTI") || name.contains("Class") || name.contains("Function ID")) {
				println("option " + name + " = " + analysis.getObject(name, null));
			}
		}
		println("compiler " + currentProgram.getCompilerSpec().getCompilerSpecID() + ", compiler name "
			+ currentProgram.getCompiler());
	}
}
