// Renames another program of the project, a closed one: its path and its new name the arguments.
// project.py build names the programs so, as pyghidra-mcp's first start did (docs/03 §3.2).
//@category OpenSacred

import ghidra.app.script.GhidraScript;
import ghidra.framework.model.DomainFile;

public class RenameProgramScript extends GhidraScript {
	@Override
	public void run() throws Exception {
		String[] args = getScriptArgs();
		DomainFile file = getState().getProject().getProjectData().getFile(args[0]);
		if (file == null) {
			throw new IllegalArgumentException("no " + args[0] + " in the project");
		}
		DomainFile renamed = file.setName(args[1]);
		println("renamed " + args[0] + " -> " + renamed.getPathname());
	}
}
