package crisp;

import java.util.Locale;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.world.level.border.WorldBorder;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.VoxelShape;

public final class WorldBorderOracle {
    static void print(final String name, final Object value) {
        System.out.println(name + " " + value);
    }

    static String bounds(final WorldBorder border, final float partial) {
        return border.getMinX(partial) + " " + border.getMinZ(partial) + " " + border.getMaxX(partial) + " " + border.getMaxZ(partial);
    }

    static String coords(final VoxelShape shape) {
        StringBuilder out = new StringBuilder();
        for (Direction.Axis axis : Direction.Axis.values()) {
            out.append(axis).append(shape.getCoords(axis)).append(' ');
        }
        out.append("boxes");
        shape.forAllBoxes((x1, y1, z1, x2, y2, z2) -> out.append(String.format(Locale.ROOT, " [%s %s %s %s %s %s]", x1, y1, z1, x2, y2, z2)));
        return out.toString();
    }

    public static void main(final String[] args) {
        WorldBorder border = new WorldBorder();
        border.applyInitialSettings(0L);
        print("default.size", border.getSize());
        print("default.bounds", bounds(border, 0.0F));
        print("default.warning", border.getWarningTime() + " " + border.getWarningBlocks());
        print("default.damage", border.getDamagePerBlock() + " " + border.getSafeZone());
        print("default.lerp", border.getLerpTime() + " " + border.getLerpTarget() + " " + border.getLerpSpeed() + " " + border.getStatus());
        print("default.shape", coords(border.getCollisionShape()));
        print("default.settings", WorldBorder.CODEC.encodeStart(NbtOps.INSTANCE, border).getOrThrow());

        border.setCenter(10.5, -3.25);
        border.setSize(100.0);
        print("static.bounds", bounds(border, 0.0F));
        print("static.shape", coords(border.getCollisionShape()));
        double[][] points = {{10.5, -3.25}, {60.0, 0.0}, {60.5, 0.0}, {61.0, 4.0}, {-40.0, -53.25}, {-50.0, 100.0}, {0.0, 46.75}};
        for (double[] p : points) {
            print("static.distance " + p[0] + " " + p[1], border.getDistanceToBorder(p[0], p[1]) + " " + border.isWithinBounds(p[0], p[1]) + " " + border.isWithinBounds(p[0], p[1], 0.5));
            print("static.clamp " + p[0] + " " + p[1], border.clampVec3ToBound(new Vec3(p[0], 64.0, p[1])) + " " + border.clampToBounds(p[0], 64.0, p[1]));
        }
        AABB[] boxes = {new AABB(59.0, 0.0, 0.0, 59.6, 1.8, 0.6), new AABB(59.7, 0.0, 0.0, 60.3, 1.8, 0.6), new AABB(60.6, 0.0, 0.0, 61.2, 1.8, 0.6), new AABB(-39.0, 0.0, -53.0, -38.4, 1.8, -52.4)};
        for (AABB box : boxes) {
            print("static.within " + box, border.isWithinBounds(box));
            VoxelShape shape = border.getCollisionShape();
            print("static.collide " + box,
                shape.collide(Direction.Axis.X, box, 5.0) + " " + shape.collide(Direction.Axis.X, box, -100.0) + " " + shape.collide(Direction.Axis.Z, box, -60.0) + " " + shape.collide(Direction.Axis.Y, box, -3.0));
        }
        print("static.within.block", border.isWithinBounds(new BlockPos(60, 0, 0)) + " " + border.isWithinBounds(new BlockPos(59, 0, 46)) + " " + border.isWithinBounds(new BlockPos(-40, 0, -54)));

        border.lerpSizeBetween(100.0, 20.0, 50L, 1000L);
        print("lerp.start", border.getSize() + " " + border.getLerpTime() + " " + border.getLerpTarget() + " " + border.getLerpSpeed() + " " + border.getStatus());
        for (int tick = 1; tick <= 52; tick++) {
            border.tick();
            print("lerp.tick " + tick, border.getSize() + " " + border.getLerpTime() + " " + border.getStatus() + " " + bounds(border, 0.0F));
            if (tick == 20) {
                print("lerp.shape", coords(border.getCollisionShape()));
                print("lerp.settings", WorldBorder.CODEC.encodeStart(NbtOps.INSTANCE, border).getOrThrow());
                print("lerp.collide", border.getCollisionShape().collide(Direction.Axis.X, new AABB(40.0, 0.0, 0.0, 40.6, 1.8, 0.6), 10.0));
            }
        }

        WorldBorder odd = new WorldBorder();
        odd.applyInitialSettings(0L);
        odd.lerpSizeBetween(7.3, 1000.1, 3L, 5L);
        for (int tick = 1; tick <= 4; tick++) {
            odd.tick();
            print("odd.tick " + tick, odd.getSize() + " " + odd.getLerpTime() + " " + bounds(odd, 0.0F));
        }
        odd.lerpSizeBetween(1000.1, 1000.1, 30L, 5L);
        print("odd.same", odd.getSize() + " " + odd.getLerpTime() + " " + odd.getStatus());
        odd.lerpSizeBetween(10.0, 30.0, 1L, 5L);
        print("odd.one", odd.getSize() + " " + odd.getLerpTime() + " " + odd.getStatus());
        odd.tick();
        print("odd.one.tick", odd.getSize() + " " + odd.getLerpTime() + " " + odd.getStatus());

        WorldBorder clamped = new WorldBorder();
        clamped.applyInitialSettings(0L);
        clamped.setCenter(40.0, 0.0);
        clamped.setSize(100.0);
        clamped.setAbsoluteMaxSize(50);
        print("clamped.bounds", bounds(clamped, 0.0F));
        print("clamped.shape", coords(clamped.getCollisionShape()));
        clamped.setAbsoluteMaxSize(1);
        clamped.setCenter(1000.0, 1000.0);
        print("degenerate.bounds", bounds(clamped, 0.0F));
        print("degenerate.shape", coords(clamped.getCollisionShape()));
        clamped.lerpSizeBetween(100.0, 50.0, 10L, 0L);
        clamped.tick();
        print("degenerate.lerp.bounds", bounds(clamped, 0.0F));

        CompoundTag far = new CompoundTag();
        far.putDouble("center_x", 3.0E7);
        far.putDouble("center_z", 0.0);
        far.putDouble("damage_per_block", 0.5);
        far.putDouble("safe_zone", 2.0);
        far.putInt("warning_blocks", 3);
        far.putInt("warning_time", 10);
        far.putDouble("size", 50.0);
        far.putLong("lerp_time", 0L);
        far.putDouble("lerp_target", 50.0);
        print("codec.far", WorldBorder.CODEC.parse(NbtOps.INSTANCE, (Tag) far).resultOrPartial(e -> print("codec.far.error", e)).map(b -> {
            b.applyInitialSettings(0L);
            return b.getCenterX() + " " + b.getSize();
        }).orElse("none"));
        CompoundTag ints = far.copy();
        ints.putInt("center_x", 7);
        ints.putFloat("size", 12.5F);
        ints.putByte("lerp_time", (byte) 4);
        ints.putShort("warning_time", (short) 9);
        print("codec.numbers", WorldBorder.CODEC.parse(NbtOps.INSTANCE, (Tag) ints).resultOrPartial(e -> print("codec.numbers.error", e)).map(b -> {
            b.applyInitialSettings(100L);
            return b.getCenterX() + " " + b.getSize() + " " + b.getLerpTime() + " " + b.getLerpTarget() + " " + b.getWarningTime();
        }).orElse("none"));
        CompoundTag missing = far.copy();
        missing.remove("safe_zone");
        print("codec.missing", WorldBorder.CODEC.parse(NbtOps.INSTANCE, (Tag) missing).resultOrPartial(e -> print("codec.missing.error", e)).map(b -> "parsed").orElse("none"));
    }
}
