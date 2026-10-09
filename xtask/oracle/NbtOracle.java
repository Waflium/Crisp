package crisp;

import com.mojang.brigadier.exceptions.CommandSyntaxException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.NbtPathArgument;
import com.mojang.serialization.JsonOps;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtUtils;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.nbt.TagParser;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.server.Bootstrap;

public class NbtOracle {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (line.isEmpty()) continue;
            String[] f = line.split("\t");
            String out;
            try {
                out = run(f);
            } catch (CommandSyntaxException e) {
                out = "ERR " + ((Component) e.getRawMessage()).getString();
            }
            System.out.println(out);
        }
    }

    static String pretty(Tag tag) {
        return ComponentSerialization.CODEC.encodeStart(JsonOps.INSTANCE, NbtUtils.toPrettyComponent(tag)).getOrThrow().toString();
    }

    static String run(String[] f) throws CommandSyntaxException {
        if (f[0].equals("pretty")) {
            return "PRETTY " + pretty(TagParser.create(NbtOps.INSTANCE).parseFully(f[1]).copy());
        }
        if (f[0].equals("merge")) {
            CompoundTag merged = TagParser.parseCompoundFully(f[1]).copy().merge(TagParser.parseCompoundFully(f[2]));
            return "MERGE " + pretty(merged);
        }
        CompoundTag root = TagParser.parseCompoundFully(f[1]).copy();
        NbtPathArgument.NbtPath path = NbtPathArgument.NbtPath.of(f[2]);
        switch (f[0]) {
            case "get": return "GET " + path.get(root);
            case "count": return "COUNT " + path.countMatching(root);
            case "set": {
                Tag value = TagParser.create(NbtOps.INSTANCE).parseFully(f[3]);
                int n = path.set(root, value);
                return "SET " + n + " " + root;
            }
            case "insert": {
                String[] a = f[3].split(" ", 2);
                ListTag values = (ListTag) TagParser.create(NbtOps.INSTANCE).parseFully(a[1]);
                List<Tag> list = new ArrayList<>();
                values.forEach(list::add);
                int n = path.insert(Integer.parseInt(a[0]), root, list);
                return "INS " + n + " " + root;
            }
            case "remove": {
                int n = path.remove(root);
                return "REM " + n + " " + root;
            }
            default: throw new IllegalArgumentException(f[0]);
        }
    }
}
