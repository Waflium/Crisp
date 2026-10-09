package crisp;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.nio.file.Files;
import java.nio.file.Path;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.nbt.TagParser;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.resources.Identifier;
import net.minecraft.server.Bootstrap;
import net.minecraft.world.level.block.entity.BlockEntity;
import net.minecraft.world.level.block.state.BlockState;

public class BlockEntityOracle {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider registries = VanillaRegistries.createWorldLookup();
        BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(registries).forEach(DataComponentInitializers.PendingComponents::apply);
        BlockPos pos = new BlockPos(1, 64, -3);
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (line.isEmpty()) continue;
            String[] f = line.split("\t");
            BlockState state = BuiltInRegistries.BLOCK.getValue(Identifier.parse(f[0])).defaultBlockState();
            CompoundTag input = TagParser.parseCompoundFully(f[1]).copy();
            var type = ((net.minecraft.world.level.block.EntityBlock) state.getBlock()).newBlockEntity(pos, state).getType();
            input.putString("id", BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(type).toString());
            BlockEntity entity = BlockEntity.loadStatic(pos, state, input, registries);
            System.out.println("CASE " + f[0] + "\t" + f[1]);
            System.out.println("SAVE " + hex(entity.saveWithFullMetadata(registries)));
            Tag components = DataComponentMap.CODEC.encodeStart(registries.createSerializationContext(NbtOps.INSTANCE), entity.collectComponents()).getOrThrow();
            System.out.println("COMPONENTS " + hex(components));
        }
    }

    static String hex(Tag tag) {
        ByteBuf buf = Unpooled.buffer();
        FriendlyByteBuf out = new FriendlyByteBuf(buf);
        out.writeNbt(tag);
        byte[] bytes = new byte[buf.readableBytes()];
        buf.readBytes(bytes);
        StringBuilder s = new StringBuilder();
        for (byte b : bytes) s.append(String.format("%02x", b));
        return s.toString();
    }
}
