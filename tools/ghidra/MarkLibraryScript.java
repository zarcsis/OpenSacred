// Tags the library code so that it is not analyzed: the functions in the ranges given as arguments,
// "TAG:start-end" in hex (end exclusive), with TAG, and the functions in a namespace under _STL
// with STL. Prints the counts by tag.
//@category OpenSacred

import java.util.Map;
import java.util.TreeMap;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Namespace;

public class MarkLibraryScript extends GhidraScript {
	@Override
	public void run() throws Exception {
		Map<String, Integer> counts = new TreeMap<>();
		String[] ranges = getScriptArgs();
		for (Function function : currentProgram.getFunctionManager().getFunctions(true)) {
			long entry = function.getEntryPoint().getOffset();
			for (String range : ranges) {
				String[] parts = range.split("[:\\-]");
				long start = Long.parseLong(parts[1], 16), end = Long.parseLong(parts[2], 16);
				if (entry >= start && entry < end) {
					function.addTag(parts[0]);
					counts.merge(parts[0], 1, Integer::sum);
				}
			}
			for (Namespace space = function.getParentNamespace(); space != null && !space.isGlobal();
					space = space.getParentNamespace()) {
				if (space.getName().equals("_STL")) {
					function.addTag("STL");
					counts.merge("STL", 1, Integer::sum);
					break;
				}
			}
		}
		println("library tags: " + counts);
	}
}
