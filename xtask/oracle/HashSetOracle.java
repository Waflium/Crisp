package crisp;

import java.util.*;

public class HashSetOracle {
    static long state;
    static int next(int bound) { state = state * 6364136223846793005L + 1442695040888963407L; return (int) ((state >>> 33) % bound); }

    public static void main(String[] args) {
        StringBuilder out = new StringBuilder();
        for (int trial = 0; trial < 400; trial++) {
            state = trial;
            HashSet<net.minecraft.core.BlockPos> set = new HashSet<>();
            int ops = 50 + next(400);
            int spread = 1 + next(64);
            for (int i = 0; i < ops; i++) {
                net.minecraft.core.BlockPos pos = new net.minecraft.core.BlockPos(9584 + next(spread), -40 + next(12), 6976 + next(spread) * (next(3) == 0 ? 31 : 1));
                if (next(5) == 0) set.remove(pos); else set.add(pos);
            }
            out.append(trial);
            for (net.minecraft.core.BlockPos p : set) out.append(' ').append(p.getX()).append(',').append(p.getY()).append(',').append(p.getZ());
            out.append('\n');
        }
        System.out.print(out);
    }
}
