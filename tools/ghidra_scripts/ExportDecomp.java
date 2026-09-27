// Decompile every function and write C output plus a function index.
// @category Reunion
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;

public class ExportDecomp extends GhidraScript {
    @Override
    protected void run() throws Exception {
        String outDir = getScriptArgs().length > 0 ? getScriptArgs()[0] : ".";
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        int ok = 0, fail = 0;
        try (PrintWriter c = new PrintWriter(new FileWriter(outDir + "/REUNION.c"));
             PrintWriter idx = new PrintWriter(new FileWriter(outDir + "/functions.tsv"))) {
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                if (monitor.isCancelled()) break;
                DecompileResults r = di.decompileFunction(f, 60, monitor);
                idx.printf("%s\t%s\t%d%n", f.getEntryPoint(), f.getName(), f.getBody().getNumAddresses());
                c.printf("// ==== %s @ %s%n", f.getName(), f.getEntryPoint());
                if (r != null && r.decompileCompleted()) {
                    c.println(r.getDecompiledFunction().getC());
                    ok++;
                } else {
                    c.println("// decompile failed: " + (r == null ? "null" : r.getErrorMessage()));
                    fail++;
                }
            }
        }
        println("decompiled " + ok + " functions, failed " + fail);
    }
}
