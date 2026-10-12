// Decompile every function into one local text file for grepping.
// Output stays in work/ (gitignored). Usage: -postScript DumpDecomp.java <out.c>
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.listing.*;
import java.io.*;

public class DumpDecomp extends GhidraScript {
    @Override
    public void run() throws Exception {
        String out = getScriptArgs()[0];
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        int n = 0;
        try (PrintWriter w = new PrintWriter(new FileWriter(out))) {
            for (Function f : currentProgram.getFunctionManager().getFunctions(true)) {
                if (monitor.isCancelled()) break;
                DecompileResults r = di.decompileFunction(f, 60, monitor);
                w.println("// ==== " + f.getName() + " @ " + f.getEntryPoint());
                if (r != null && r.decompileCompleted())
                    w.println(r.getDecompiledFunction().getC());
                n++;
            }
        }
        println("decompiled " + n + " functions -> " + out);
    }
}
