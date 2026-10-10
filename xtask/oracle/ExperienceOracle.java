package crisp;

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.network.syncher.EntityDataAccessor;
import net.minecraft.network.syncher.SynchedEntityData;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.util.Mth;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.ExperienceOrb;
import net.minecraft.world.entity.Pose;
import net.minecraft.world.entity.player.Player;
import sun.misc.Unsafe;

public class ExperienceOracle {
   public static void main(String[] args) throws Exception {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      ServerPlayer p = player();
      String[] ops = {
         "p 5", "p 10", "p 100", "p 1000", "p 12345", "p -50", "p -3000", "l 7", "l 25", "l -3", "s 5", "p 1", "l -1000", "p 3", "p -10",
         "l 2147483647", "l 5", "p 2147483647", "l -2147483647", "l 31", "s 200", "s 0", "p 7"
      };
      for (String op : ops) {
         String[] parts = op.split(" ");
         int amount = Integer.parseInt(parts[1]);
         switch (parts[0]) {
            case "p" -> p.giveExperiencePoints(amount);
            case "l" -> p.giveExperienceLevels(amount);
            case "s" -> {
               if (amount < p.getXpNeededForNextLevel()) p.setExperiencePoints(amount);
            }
         }
         System.out.println(op + " -> level " + p.experienceLevel + " progress " + Float.floatToIntBits(p.experienceProgress) + " (" + p.experienceProgress + ") total "
            + p.totalExperience + " score " + p.getScore() + " points " + Mth.floor(p.experienceProgress * p.getXpNeededForNextLevel()));
      }
      int[] amounts = {1, 2, 3, 6, 7, 16, 17, 100, 2476, 2477, 5000, 123456};
      for (int amount : amounts) {
         StringBuilder orbs = new StringBuilder();
         for (int left = amount; left > 0; ) {
            int value = ExperienceOrb.getExperienceValue(left);
            left -= value;
            orbs.append(value).append(left > 0 ? "," : "");
         }
         System.out.println("award " + amount + " -> " + orbs);
      }
   }

   @SuppressWarnings("unchecked")
   static ServerPlayer player() throws Exception {
      Field f = Unsafe.class.getDeclaredField("theUnsafe");
      f.setAccessible(true);
      Unsafe unsafe = (Unsafe) f.get(null);
      ServerPlayer p = (ServerPlayer) unsafe.allocateInstance(ServerPlayer.class);
      SynchedEntityData.Builder builder = new SynchedEntityData.Builder(p);
      Object[][] base = {
         {"DATA_SHARED_FLAGS_ID", (byte) 0}, {"DATA_AIR_SUPPLY_ID", 300}, {"DATA_CUSTOM_NAME_VISIBLE", false}, {"DATA_CUSTOM_NAME", Optional.empty()},
         {"DATA_SILENT", false}, {"DATA_NO_GRAVITY", false}, {"DATA_POSE", Pose.STANDING}, {"DATA_TICKS_FROZEN", 0}
      };
      for (Object[] entry : base) {
         Field accessor = Entity.class.getDeclaredField((String) entry[0]);
         accessor.setAccessible(true);
         builder.define((EntityDataAccessor<Object>) accessor.get(null), entry[1]);
      }
      Method define = Player.class.getDeclaredMethod("defineSynchedData", SynchedEntityData.Builder.class);
      define.setAccessible(true);
      define.invoke(p, builder);
      Field data = Entity.class.getDeclaredField("entityData");
      unsafe.putObject(p, unsafe.objectFieldOffset(data), builder.build());
      return p;
   }
}
