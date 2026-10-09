package crisp;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.UUID;
import net.minecraft.SharedConstants;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.data.registries.VanillaRegistries;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtIo;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.util.datafix.DataFixTypes;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntityTypes;
import net.minecraft.world.entity.item.ItemEntity;
import net.minecraft.world.entity.item.PrimedTnt;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.chunk.storage.EntityStorage;
import net.minecraft.world.level.chunk.storage.RegionStorageInfo;
import net.minecraft.world.level.chunk.storage.SimpleRegionStorage;
import net.minecraft.world.level.entity.ChunkEntities;
import sun.misc.Unsafe;

public class EntityChunkOracle {
    public static class OracleLevel extends ServerLevel {
        static RegistryAccess registries;

        OracleLevel() {
            super(null, null, null, null, null, null, false, 0L, null, false);
        }

        @Override
        public int getNextEntityId() {
            return 1;
        }

        @Override
        public RegistryAccess registryAccess() {
            return registries;
        }
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        HolderLookup.Provider lookup = VanillaRegistries.createWorldLookup();
        BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(lookup).forEach(DataComponentInitializers.PendingComponents::apply);
        OracleLevel.registries = RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY);
        Field theUnsafe = Unsafe.class.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        OracleLevel level = (OracleLevel) ((Unsafe) theUnsafe.get(null)).allocateInstance(OracleLevel.class);

        ItemEntity diamonds = new ItemEntity(EntityTypes.ITEM, level);
        diamonds.setUUID(new UUID(0x1111111122224333L, 0x8444555566667777L));
        diamonds.setPos(17.5, -60.0, 3.25);
        diamonds.setDeltaMovement(0.1, 0.0, -0.05);
        diamonds.setYRot(90.0F);
        Field onGround = Entity.class.getDeclaredField("onGround");
        onGround.setAccessible(true);
        onGround.setBoolean(diamonds, true);
        diamonds.setItem(new ItemStack(Items.DIAMOND, 3));
        diamonds.setPickUpDelay(10);

        ItemEntity stone = new ItemEntity(EntityTypes.ITEM, level);
        stone.setUUID(new UUID(0x0123456789ab4cdeL, 0x8f0123456789abcdL));
        stone.setPos(20.0, -59.0, 6.0);
        stone.setYRot(12.5F);
        stone.setItem(new ItemStack(Items.STONE, 64));

        PrimedTnt tnt = new PrimedTnt(EntityTypes.TNT, level);
        tnt.setUUID(new UUID(-0x5555555555555556L, -0x3333333333333334L));
        tnt.setPos(18.5, -60.0, 10.5);
        tnt.setDeltaMovement(0.0, 0.2, 0.0);
        tnt.setFuse(33);

        Path dir = Files.createTempDirectory("crisp-entity-oracle");
        SimpleRegionStorage regions = new SimpleRegionStorage(
            new RegionStorageInfo("oracle", Level.OVERWORLD, "entities"), dir, null, true, DataFixTypes.ENTITY_CHUNK
        );
        EntityStorage storage = new EntityStorage(regions, level, Runnable::run);
        ChunkPos pos = new ChunkPos(1, 0);
        storage.storeEntities(new ChunkEntities<Entity>(pos, List.of(diamonds, stone, tnt)));
        storage.flush(true);
        CompoundTag stored = regions.read(pos).join().orElseThrow();
        System.out.println("STORED " + hex(stored));
        storage.storeEntities(new ChunkEntities<Entity>(pos, List.of()));
        storage.flush(true);
        System.out.println("EMPTY " + regions.read(pos).join().isPresent());
        storage.close();
    }

    static String hex(CompoundTag tag) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        NbtIo.write(tag, new DataOutputStream(bytes));
        StringBuilder s = new StringBuilder();
        for (byte b : bytes.toByteArray()) s.append(String.format("%02x", b));
        return s.toString();
    }
}
