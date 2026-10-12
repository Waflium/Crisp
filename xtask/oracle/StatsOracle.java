package crisp;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.datafixers.DataFixer;
import it.unimi.dsi.fastutil.objects.Object2IntMap;
import it.unimi.dsi.fastutil.objects.Object2IntOpenHashMap;
import java.lang.reflect.Method;
import java.nio.file.Path;
import java.util.Map;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.server.Bootstrap;
import net.minecraft.stats.ServerStatsCounter;
import net.minecraft.stats.Stat;
import net.minecraft.stats.StatType;
import net.minecraft.util.datafix.DataFixers;

public class StatsOracle {
    static long state;
    static Method getDirty;
    static Method toJson;

    static int next(int bound) {
        state = state * 6364136223846793005L + 1442695040888963407L;
        return (int) ((state >>> 33) % bound);
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Stat<?> randomStat() {
        StatType type = BuiltInRegistries.STAT_TYPE.byId(next(9));
        int size = type.getRegistry().size();
        int spread = Math.min(size, 60);
        return type.get(type.getRegistry().byId(next(spread)));
    }

    @SuppressWarnings("unchecked")
    static void send(ServerStatsCounter c, StringBuilder out) throws Exception {
        Set<Stat<?>> dirty = (Set<Stat<?>>) getDirty.invoke(c);
        Object2IntMap<Stat<?>> toSend = new Object2IntOpenHashMap<>();
        for (Stat<?> stat : dirty) {
            toSend.put(stat, c.getValue(stat));
        }
        out.append(" S");
        toSend.forEach((k, v) -> out.append(' ').append(k.getName()).append('=').append(v));
    }

    static JsonObject json(ServerStatsCounter c, StringBuilder out) throws Exception {
        JsonObject root = (JsonObject) toJson.invoke(c);
        JsonObject stats = root.getAsJsonObject("stats");
        JsonObject ordered = new JsonObject();
        for (StatType<?> type : BuiltInRegistries.STAT_TYPE) {
            String key = BuiltInRegistries.STAT_TYPE.getKey(type).toString();
            if (!stats.has(key)) {
                continue;
            }
            JsonObject inner = stats.getAsJsonObject(key);
            ordered.add(key, inner);
            out.append(" J ").append(key);
            for (Map.Entry<String, JsonElement> e : inner.entrySet()) {
                out.append(' ').append(e.getKey()).append('=').append(e.getValue().getAsInt());
            }
        }
        JsonObject result = new JsonObject();
        result.add("stats", ordered);
        result.add("DataVersion", root.get("DataVersion"));
        return result;
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        getDirty = ServerStatsCounter.class.getDeclaredMethod("getDirty");
        getDirty.setAccessible(true);
        toJson = ServerStatsCounter.class.getDeclaredMethod("toJson");
        toJson.setAccessible(true);
        DataFixer fixer = DataFixers.getDataFixer();
        int trials = Integer.parseInt(args[0]);
        for (int trial = 0; trial < trials; trial++) {
            state = trial;
            StringBuilder out = new StringBuilder("T").append(trial);
            ServerStatsCounter c = new ServerStatsCounter(null, Path.of("nonexistent-stats.json"));
            int ops = 1 + next(150);
            for (int i = 0; i < ops; i++) {
                int kind = next(12);
                if (kind == 0) {
                    send(c, out);
                } else if (kind == 1) {
                    c.markAllDirty();
                    out.append(" M");
                } else {
                    Stat<?> stat = randomStat();
                    c.increment(null, stat, 1 + next(5));
                }
            }
            send(c, out);
            JsonObject saved = json(c, out);
            ServerStatsCounter d = new ServerStatsCounter(null, Path.of("nonexistent-stats.json"));
            d.parse(fixer, saved);
            out.append(" R");
            d.markAllDirty();
            send(d, out);
            int more = next(20);
            for (int i = 0; i < more; i++) {
                d.increment(null, randomStat(), 1 + next(5));
            }
            send(d, out);
            json(d, out);
            System.out.println(out);
        }
    }
}
