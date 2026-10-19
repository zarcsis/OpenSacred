// Prints a program's counts: functions by the source of their names, those marked as library
// functions (tag LIBRARY or a Function ID bookmark), classes, vftables.
//@category OpenSacred

import java.util.Map;
import java.util.TreeMap;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Bookmark;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionTag;
import ghidra.program.model.symbol.Namespace;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;
import ghidra.program.model.symbol.SymbolType;

public class ProjectStatsScript extends GhidraScript {
	@Override
	public void run() throws Exception {
		Map<String, Integer> bySource = new TreeMap<>();
		int total = 0, library = 0, thunks = 0, inClasses = 0;
		for (Function function : currentProgram.getFunctionManager().getFunctions(true)) {
			total++;
			bySource.merge(function.getSymbol().getSource().toString(), 1, Integer::sum);
			if (function.isThunk()) {
				thunks++;
			}
			boolean lib = false;
			for (FunctionTag tag : function.getTags()) {
				if (tag.getName().toUpperCase().contains("LIBRARY")) {
					lib = true;
				}
			}
			Bookmark[] marks = currentProgram.getBookmarkManager().getBookmarks(function.getEntryPoint());
			for (Bookmark mark : marks) {
				if (mark.getCategory().contains("Function ID")) {
					lib = true;
				}
			}
			if (lib) {
				library++;
			}
			Namespace parent = function.getParentNamespace();
			if (parent != null && parent.getSymbol().getSymbolType() == SymbolType.CLASS) {
				inClasses++;
			}
		}
		int classes = 0, vftables = 0;
		var classSpaces = currentProgram.getSymbolTable().getClassNamespaces();
		while (classSpaces.hasNext()) {
			classSpaces.next();
			classes++;
		}
		SymbolIterator it = currentProgram.getSymbolTable().getAllSymbols(false);
		while (it.hasNext()) {
			Symbol symbol = it.next();
			if (symbol.getName().contains("vftable")) {
				vftables++;
			}
		}
		println(String.format("stats %s: %d functions (%d thunks) by source %s; %d library; %d in classes; %d classes; %d vftable symbols",
			currentProgram.getName(), total, thunks, bySource, library, inClasses, classes, vftables));
	}
}
