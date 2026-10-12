package crisp;

import java.lang.reflect.Array;
import java.lang.reflect.Field;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.List;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Holder;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.commands.SpreadPlayersCommand;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.tags.BlockTags;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.BlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.phys.Vec2;
import sun.misc.Unsafe;

public class SpreadPlayersOracle {
    public static class OracleLevel extends ServerLevel {
        OracleLevel() {
            super(null, null, null, null, null, null, false, 0L, null, false);
        }

        @Override
        public BlockState getBlockState(BlockPos pos) {
            return terrain(pos.getX(), pos.getY(), pos.getZ());
        }

        @Override
        public int getMinY() {
            return -64;
        }
    }

    static BlockState terrain(int x, int y, int z) {
        if (y < -64 || y > 319) {
            return Blocks.VOID_AIR.defaultBlockState();
        }
        if (x >= -1 && x <= 0 && z >= -1 && z <= 0 && y >= -60 && y <= -50) {
            return Blocks.STONE.defaultBlockState();
        }
        if (y == -64) {
            return Blocks.BEDROCK.defaultBlockState();
        }
        if (y < -61) {
            return Blocks.DIRT.defaultBlockState();
        }
        if (y == -61) {
            return x >= 3 && x <= 6 && z >= 3 && z <= 6 ? Blocks.WATER.defaultBlockState() : Blocks.GRASS_BLOCK.defaultBlockState();
        }
        return Blocks.AIR.defaultBlockState();
    }

    @SuppressWarnings("unchecked")
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        Method bindTags = Holder.Reference.class.getDeclaredMethod("bindTags", java.util.Collection.class);
        bindTags.setAccessible(true);
        for (Block block : List.of(Blocks.STONE, Blocks.BEDROCK, Blocks.DIRT, Blocks.GRASS_BLOCK)) {
            bindTags.invoke(block.builtInRegistryHolder(), List.of(BlockTags.ENTITIES_CAN_TELEPORT_TO));
        }
        Field theUnsafe = Unsafe.class.getDeclaredField("theUnsafe");
        theUnsafe.setAccessible(true);
        OracleLevel level = (OracleLevel) ((Unsafe) theUnsafe.get(null)).allocateInstance(OracleLevel.class);

        Class<?> position = Class.forName("net.minecraft.server.commands.SpreadPlayersCommand$Position");
        Method createInitialPositions = SpreadPlayersCommand.class.getDeclaredMethod(
            "createInitialPositions", RandomSource.class, int.class, double.class, double.class, double.class, double.class
        );
        createInitialPositions.setAccessible(true);
        Class<?> positionArray = Array.newInstance(position, 0).getClass();
        Method spreadPositions = SpreadPlayersCommand.class.getDeclaredMethod(
            "spreadPositions", Vec2.class, double.class, ServerLevel.class, RandomSource.class, double.class, double.class, double.class, double.class,
            int.class, positionArray, boolean.class
        );
        spreadPositions.setAccessible(true);
        Method getSpawnY = position.getDeclaredMethod("getSpawnY", BlockGetter.class, int.class);
        getSpawnY.setAccessible(true);
        Field fx = position.getDeclaredField("x");
        Field fz = position.getDeclaredField("z");
        fx.setAccessible(true);
        fz.setAccessible(true);

        Object[][] cases = {
            {1L, 0.5F, 0.5F, 2.0F, 10.0F, 320, 5},
            {42L, 10.5F, -3.5F, 5.0F, 8.0F, 320, 8},
            {7L, 0.5F, 0.5F, 50.0F, 5.0F, 320, 4},
            {99L, 0.5F, 0.5F, 0.0F, 1.0F, -55, 3},
            {123456789L, 3.5F, 4.5F, 3.0F, 3.0F, 320, 2},
            {5L, 4.5F, 4.5F, 1.0F, 2.0F, 320, 6},
        };
        for (int i = 0; i < cases.length; i++) {
            Object[] c = cases[i];
            RandomSource random = RandomSource.createThreadLocalInstance((Long) c[0]);
            Vec2 center = new Vec2((Float) c[1], (Float) c[2]);
            float spreadDistance = (Float) c[3];
            float maxRange = (Float) c[4];
            int maxHeight = (Integer) c[5];
            int count = (Integer) c[6];
            double minX = center.x - maxRange;
            double minZ = center.y - maxRange;
            double maxX = center.x + maxRange;
            double maxZ = center.y + maxRange;
            Object positions = createInitialPositions.invoke(null, random, count, minX, minZ, maxX, maxZ);
            StringBuilder line = new StringBuilder("case " + i);
            try {
                spreadPositions.invoke(null, center, (double) spreadDistance, level, random, minX, minZ, maxX, maxZ, maxHeight, positions, false);
                for (int p = 0; p < count; p++) {
                    Object pos = Array.get(positions, p);
                    line.append(' ').append(Double.doubleToLongBits(fx.getDouble(pos))).append(',').append(Double.doubleToLongBits(fz.getDouble(pos)))
                        .append(',').append(getSpawnY.invoke(pos, level, maxHeight));
                }
            } catch (InvocationTargetException e) {
                line.append(" error ").append(((com.mojang.brigadier.exceptions.CommandSyntaxException) e.getCause()).getRawMessage().getString());
            }
            line.append(" next ").append(random.nextLong());
            System.out.println(line);
        }
    }
}
