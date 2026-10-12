// Names the Vicious Engine class loaders in the PS2 ELF (SLES_530.78) from notes/class-map.csv.
//
// For every row with confidence "high" or "medium" and a ps2_loader address, the function at that
// address is renamed to <ClassName>_Deserialize, but only while it still has a default name
// (FUN_xxxxxxxx or no function yet). Names someone chose by hand are never overwritten.
// A plate comment with the class id, the versions seen in the PS2 data and the vicious schema
// file is added (once; re-running the script does not duplicate it).
//
// GUI:      Script Manager -> ApplyClassMap.java (asks for the CSV if it is not found next to the
//           project's notes/ folder).
// Headless: analyzeHeadless <projdir> <proj> -process SLES_530.78 -noanalysis
//             -scriptPath tools/ghidra -postScript ApplyClassMap.java notes/class-map.csv
//
// Optional second argument (headless) "all" also applies rows with confidence "low".
//@category SpyVsSpy
//@menupath Tools.SpyVsSpy.Apply Class Map

import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.SourceType;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

public class ApplyClassMap extends GhidraScript {

    private static final String MARKER = "[class-map]";

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File csv = null;
        if (args.length > 0) {
            csv = new File(args[0]);
        } else {
            File guess = new File(getSourceFile().getFile(false).getParentFile().getParentFile()
                .getParentFile(), "notes/class-map.csv");
            csv = guess.exists() ? guess : askFile("class-map.csv", "Use");
        }
        boolean includeLow = args.length > 1 && args[1].equalsIgnoreCase("all");

        List<String[]> rows = new ArrayList<>();
        Map<String, Integer> col = new HashMap<>();
        try (BufferedReader r = new BufferedReader(new FileReader(csv))) {
            String[] head = split(r.readLine());
            for (int i = 0; i < head.length; i++) col.put(head[i].trim(), i);
            String line;
            while ((line = r.readLine()) != null) {
                if (!line.isBlank()) rows.add(split(line));
            }
        }
        for (String need : new String[] {"class_id", "class_name", "vicious_file", "ps2_loader",
                "ps2_versions_seen", "confidence"}) {
            if (!col.containsKey(need)) throw new IllegalArgumentException("CSV lacks column " + need);
        }

        int renamed = 0, created = 0, keptUser = 0, alreadyOurs = 0, comments = 0, failed = 0,
            skippedConf = 0, noLoader = 0;
        List<String> userNamed = new ArrayList<>();

        for (String[] row : rows) {
            String conf = get(row, col, "confidence");
            String loader = get(row, col, "ps2_loader");
            if (loader.isEmpty()) { noLoader++; continue; }
            boolean ok = conf.equals("high") || conf.equals("medium") || (includeLow && conf.equals("low"));
            if (!ok) { skippedConf++; continue; }

            String cls = get(row, col, "class_name");
            String id = get(row, col, "class_id");
            String want = cls + "_Deserialize";
            Address a = toAddr(Long.parseLong(loader.replace("0x", ""), 16));

            Function f = getFunctionAt(a);
            if (f == null) {
                // Loaders are often reached only through vtables, so auto-analysis may have missed them.
                if (getInstructionAt(a) == null) disassemble(a);
                if (new CreateFunctionCmd(a).applyTo(currentProgram, monitor)) created++;
                f = getFunctionAt(a);
            }
            if (f == null) { failed++; printerr("could not create a function at " + a + " for " + cls); continue; }

            String cur = f.getName();
            if (cur.equals(want)) {
                alreadyOurs++;
            } else if (f.getSymbol().getSource() == SourceType.DEFAULT || cur.startsWith("FUN_")) {
                f.setName(want, SourceType.ANALYSIS);
                renamed++;
            } else {
                keptUser++;
                userNamed.add(a + " " + cur + " (class " + cls + ")");
            }

            String plate = MARKER + " Vicious class " + cls + " (id " + id + "), confidence " + conf + "\n"
                + "PS2 data versions: " + orNone(get(row, col, "ps2_versions_seen")) + "\n"
                + "vicious schema: packages/core/src/schema/types/" + get(row, col, "vicious_file")
                + (col.containsKey("xbox_offset") && !get(row, col, "xbox_offset").isEmpty()
                    ? " (Xbox loader " + get(row, col, "xbox_offset") + ")" : "");
            String old = getPlateComment(a);
            if (old == null || !old.contains(MARKER)) {
                setPlateComment(a, old == null || old.isEmpty() ? plate : old + "\n" + plate);
                comments++;
            }
        }

        println("ApplyClassMap: " + rows.size() + " rows from " + csv);
        println("  renamed to <Class>_Deserialize: " + renamed);
        println("  functions created first:        " + created);
        println("  already named by this script:   " + alreadyOurs);
        println("  kept user-chosen names:         " + keptUser);
        println("  plate comments added:           " + comments);
        println("  rows skipped (confidence):      " + skippedConf);
        println("  rows without a PS2 loader:      " + noLoader);
        println("  failures:                       " + failed);
        for (String u : userNamed) println("  kept: " + u);
    }

    private static String orNone(String s) { return s.isEmpty() ? "none found" : s; }

    private static String get(String[] row, Map<String, Integer> col, String name) {
        Integer i = col.get(name);
        return (i == null || i >= row.length) ? "" : row[i].trim();
    }

    // Minimal CSV splitter: handles double-quoted fields with "" escapes.
    private static String[] split(String line) {
        List<String> out = new ArrayList<>();
        StringBuilder sb = new StringBuilder();
        boolean q = false;
        for (int i = 0; i < line.length(); i++) {
            char c = line.charAt(i);
            if (q) {
                if (c == '"' && i + 1 < line.length() && line.charAt(i + 1) == '"') { sb.append('"'); i++; }
                else if (c == '"') q = false;
                else sb.append(c);
            } else if (c == '"') {
                q = true;
            } else if (c == ',') {
                out.add(sb.toString()); sb.setLength(0);
            } else {
                sb.append(c);
            }
        }
        out.add(sb.toString());
        return out.toArray(new String[0]);
    }
}
