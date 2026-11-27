// Writes a program's markup as sorted text lines into <dir>/<executable's file name>.<tag>.dump
// (the arguments dir and tag), so that two programs of one executable compare line by line
// (project.py compare): the functions with their signatures, frames, tags and variables, the
// comments, the symbols, the defined data, the equates, the bookmarks, the data types with their
// members, the references' counts by type and source and the user's references, the function
// tags. The dump stays outside the repository (docs/08 §3).
//@category OpenSacred

import java.io.BufferedWriter;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressIterator;
import ghidra.program.model.data.Composite;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeComponent;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.Enum;
import ghidra.program.model.data.FunctionDefinition;
import ghidra.program.model.data.TypeDef;
import ghidra.program.model.listing.Bookmark;
import ghidra.program.model.listing.CommentType;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionTag;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Variable;
import ghidra.program.model.symbol.Equate;
import ghidra.program.model.symbol.EquateReference;
import ghidra.program.model.symbol.Reference;
import ghidra.program.model.symbol.ReferenceIterator;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolIterator;

public class DumpMarkupScript extends GhidraScript {
	private final List<String> lines = new ArrayList<>();

	private static String esc(String text) {
		if (text == null) {
			return "";
		}
		return text.replace("\\", "\\\\").replace("\n", "\\n").replace("\r", "\\r").replace("\t", "\\t");
	}

	private void add(String... fields) {
		lines.add(String.join("\t", fields));
	}

	@Override
	public void run() throws Exception {
		String[] args = getScriptArgs();
		File dir = new File(args[0]);
		String tag = args[1];
		String exe = new File(currentProgram.getExecutablePath()).getName();
		Listing listing = currentProgram.getListing();

		add("0P", currentProgram.getLanguageID().toString(),
			currentProgram.getCompilerSpec().getCompilerSpecID().toString(),
			currentProgram.getImageBase().toString());
		add("0P", "instructions", Long.toString(listing.getNumInstructions()));
		add("0P", "defined data", Long.toString(listing.getNumDefinedData()));
		add("0P", "functions", Integer.toString(currentProgram.getFunctionManager().getFunctionCount()));

		for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
			dumpFunction("F", f);
		}
		for (Function f : currentProgram.getFunctionManager().getExternalFunctions()) {
			dumpFunction("FX", f);
		}

		AddressIterator comments = listing.getCommentAddressIterator(currentProgram.getMemory(), true);
		while (comments.hasNext()) {
			Address a = comments.next();
			for (CommentType type : CommentType.values()) {
				String c = listing.getComment(type, a);
				if (c != null) {
					add("C", a.toString(), type.name(), esc(c));
				}
			}
		}

		SymbolIterator symbols = currentProgram.getSymbolTable().getAllSymbols(false);
		while (symbols.hasNext()) {
			Symbol s = symbols.next();
			add("S", s.getAddress().toString(), s.getSymbolType().toString(), esc(s.getName(true)),
				s.getSource().toString(), s.isPrimary() ? "primary" : "", s.isPinned() ? "pinned" : "");
		}

		for (Data d : listing.getDefinedData(true)) {
			add("D", d.getAddress().toString(), esc(d.getDataType().getPathName()),
				Integer.toString(d.getLength()));
		}

		Iterator<Equate> equates = currentProgram.getEquateTable().getEquates();
		while (equates.hasNext()) {
			Equate e = equates.next();
			List<String> refs = new ArrayList<>();
			for (EquateReference r : e.getReferences()) {
				refs.add(r.getAddress() + ":" + r.getOpIndex());
			}
			Collections.sort(refs);
			add("E", esc(e.getName()), Long.toString(e.getValue()), String.join(",", refs));
		}

		Iterator<Bookmark> bookmarks = currentProgram.getBookmarkManager().getBookmarksIterator();
		while (bookmarks.hasNext()) {
			Bookmark b = bookmarks.next();
			add("B", b.getAddress().toString(), b.getTypeString(), esc(b.getCategory()),
				esc(b.getComment()));
		}

		DataTypeManager dtm = currentProgram.getDataTypeManager();
		Iterator<DataType> types = dtm.getAllDataTypes();
		while (types.hasNext()) {
			DataType t = types.next();
			String path = esc(t.getPathName());
			add("T", path, t.getClass().getSimpleName(), Integer.toString(t.getLength()),
				esc(t.getDescription()));
			if (t instanceof Composite comp) {
				for (DataTypeComponent m : comp.getDefinedComponents()) {
					add("T", path, "member", Integer.toString(m.getOffset()), esc(m.getFieldName()),
						esc(m.getDataType().getPathName()), Integer.toString(m.getLength()),
						esc(m.getComment()));
				}
			}
			else if (t instanceof Enum en) {
				for (String n : en.getNames()) {
					add("T", path, "value", esc(n), Long.toString(en.getValue(n)));
				}
			}
			else if (t instanceof TypeDef td) {
				add("T", path, "base", esc(td.getDataType().getPathName()));
			}
			else if (t instanceof FunctionDefinition fd) {
				add("T", path, "prototype", esc(fd.getPrototypeString(true)));
			}
		}

		Map<String, Integer> refCounts = new TreeMap<>();
		ReferenceIterator refs = currentProgram.getReferenceManager()
				.getReferenceIterator(currentProgram.getMinAddress());
		while (refs.hasNext()) {
			Reference r = refs.next();
			refCounts.merge(r.getReferenceType() + " " + r.getSource(), 1, Integer::sum);
			if (r.getSource() == SourceType.USER_DEFINED) {
				add("RU", r.getFromAddress().toString(), Integer.toString(r.getOperandIndex()),
					r.getToAddress().toString(), r.getReferenceType().toString(),
					r.isPrimary() ? "primary" : "");
			}
		}
		for (Map.Entry<String, Integer> e : refCounts.entrySet()) {
			add("RC", e.getKey(), Integer.toString(e.getValue()));
		}

		for (FunctionTag t : currentProgram.getFunctionManager().getFunctionTagManager()
				.getAllFunctionTags()) {
			add("G", esc(t.getName()), esc(t.getComment()));
		}

		Collections.sort(lines);
		File out = new File(dir, exe + "." + tag + ".dump");
		try (BufferedWriter w = Files.newBufferedWriter(out.toPath(), StandardCharsets.UTF_8)) {
			for (String line : lines) {
				w.write(line);
				w.write("\n");
			}
		}
		println("dump " + currentProgram.getName() + " -> " + out + " (" + lines.size() + " lines)");
	}

	private void dumpFunction(String kind, Function f) {
		String entry = f.getEntryPoint().toString();
		List<String> tags = new ArrayList<>();
		for (FunctionTag t : f.getTags()) {
			tags.add(t.getName());
		}
		Collections.sort(tags);
		Function thunked = f.isThunk() ? f.getThunkedFunction(false) : null;
		add(kind, entry, esc(f.getName(true)), f.getSymbol().getSource().toString(),
			esc(f.getPrototypeString(true, true)), esc(f.getCallingConventionName()),
			f.hasNoReturn() ? "noreturn" : "", f.isInline() ? "inline" : "",
			f.hasCustomVariableStorage() ? "custom" : "", f.hasVarArgs() ? "varargs" : "",
			thunked == null ? "" : "thunk->" + thunked.getEntryPoint(),
			"purge=" + f.getStackPurgeSize(), "locals=" + f.getStackFrame().getLocalSize(),
			f.getBody().isEmpty() ? "body="
				: "body=" + f.getBody().getMinAddress() + "-" + f.getBody().getMaxAddress() + "/"
					+ f.getBody().getNumAddressRanges() + "/" + f.getBody().getNumAddresses(),
			"tags=" + String.join(",", tags), esc(f.getComment()), esc(f.getRepeatableComment()));
		for (Variable v : f.getAllVariables()) {
			add(kind + "V", entry, esc(v.getName()), esc(v.getVariableStorage().toString()),
				esc(v.getDataType().getPathName()), v.getSource().toString(), esc(v.getComment()));
		}
	}
}
