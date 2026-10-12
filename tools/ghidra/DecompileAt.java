// Decompile functions starting at given addresses (creating them if Ghidra missed them).
// Usage: -postScript DecompileAt.java <out.c> <hexaddr> [<hexaddr>...]
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.*;
import java.io.*;

public class DecompileAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        DecompInterface di = new DecompInterface();
        di.openProgram(currentProgram);
        try (PrintWriter w = new PrintWriter(new FileWriter(args[0]))) {
            for (int i = 1; i < args.length; i++) {
                Address a = toAddr(Long.parseLong(args[i], 16));
                Function f = getFunctionAt(a);
                if (f == null) {
                    Function old = getFunctionContaining(a);
                    if (old != null) removeFunction(old);
                    disassemble(a);
                    new CreateFunctionCmd(a).applyTo(currentProgram, monitor);
                    f = getFunctionAt(a);
                }
                w.println("// ==== " + args[i]);
                if (f == null) { w.println("// could not create function"); continue; }
                DecompileResults r = di.decompileFunction(f, 120, monitor);
                w.println(r.decompileCompleted() ? r.getDecompiledFunction().getC() : "// decompile failed");
            }
        }
    }
}
