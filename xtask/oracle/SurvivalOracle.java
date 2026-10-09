package crisp;

import net.minecraft.SharedConstants;
import net.minecraft.core.Holder;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.Mth;
import net.minecraft.world.damagesource.CombatRules;
import net.minecraft.world.damagesource.DamageSource;
import net.minecraft.world.damagesource.DamageType;
import net.minecraft.world.food.FoodData;
import net.minecraft.world.food.FoodProperties;
import net.minecraft.world.level.levelgen.LegacyRandomSource;
import net.minecraft.world.phys.Vec3;

public class SurvivalOracle {
   public static void main(String[] args) {
      SharedConstants.tryDetectVersion();
      Bootstrap.bootStrap();
      DamageSource source = new DamageSource((Holder<DamageType>) null);
      float[][] cases = {{20.0F, 20.0F, 8.0F}, {10.0F, 20.0F, 8.0F}, {5.0F, 15.0F, 0.0F}, {4.0F, 7.0F, 0.0F}, {1.0F, 20.0F, 12.0F}, {50.0F, 20.0F, 8.0F}, {7.0F, 20.0F, 8.0F}, {6.0F, 3.0F, 0.0F}};
      for (float[] c : cases) {
         System.out.println("absorb " + c[0] + " " + c[1] + " " + c[2] + " -> " + Float.floatToIntBits(CombatRules.getDamageAfterAbsorb(null, c[0], source, c[1], c[2])) + " " + CombatRules.getDamageAfterAbsorb(null, c[0], source, c[1], c[2]));
      }
      FoodProperties beef = new FoodProperties(8, 12.8F, false);
      FoodData food = new FoodData();
      food.setFoodLevel(10);
      food.setSaturation(0.0F);
      food.eat(beef);
      System.out.println("after beef " + food.getFoodLevel() + " " + food.getSaturationLevel());
      food.eat(beef);
      System.out.println("after 2 beef " + food.getFoodLevel() + " " + food.getSaturationLevel());

      LegacyRandomSource random = new LegacyRandomSource(42L);
      double power = 0.4F;
      double xd = 0.0;
      double zd = 0.0;
      while (xd * xd + zd * zd < 1.0E-5F) {
         xd = (random.nextDouble() - random.nextDouble()) * 0.01;
         zd = (random.nextDouble() - random.nextDouble()) * 0.01;
      }
      Vec3 movement = new Vec3(0.0, -0.0784000015258789, 0.0);
      Vec3 delta = new Vec3(xd, 0.0, zd).normalize().scale(power);
      Vec3 result = new Vec3(movement.x / 2.0 - delta.x, Math.min(0.4, movement.y / 2.0 + power), movement.z / 2.0 - delta.z);
      System.out.println("knockback " + Double.doubleToLongBits(result.x) + " " + Double.doubleToLongBits(result.y) + " " + Double.doubleToLongBits(result.z) + " " + result);

      float health = 10.0F;
      int foodLevel = 20;
      float saturation = 5.0F;
      float exhaustion = 0.0F;
      int timer = 0;
      StringBuilder log = new StringBuilder();
      for (int tick = 1; tick <= 200; tick++) {
         if (exhaustion > 4.0F) {
            exhaustion -= 4.0F;
            if (saturation > 0.0F) {
               saturation = Math.max(saturation - 1.0F, 0.0F);
            } else {
               foodLevel = Math.max(foodLevel - 1, 0);
            }
         }
         boolean hurt = health > 0.0F && health < 20.0F;
         if (saturation > 0.0F && hurt && foodLevel >= 20) {
            if (++timer >= 10) {
               float spent = Math.min(saturation, 6.0F);
               health = Mth.clamp(health + spent / 6.0F, 0.0F, 20.0F);
               exhaustion = Math.min(exhaustion + spent, 40.0F);
               timer = 0;
               log.append(tick).append(':').append(Float.floatToIntBits(health)).append(' ');
            }
         } else if (foodLevel >= 18 && hurt) {
            if (++timer >= 80) {
               health = Mth.clamp(health + 1.0F, 0.0F, 20.0F);
               exhaustion = Math.min(exhaustion + 6.0F, 40.0F);
               timer = 0;
               log.append(tick).append(':').append(Float.floatToIntBits(health)).append(' ');
            }
         } else if (foodLevel <= 0) {
            timer++;
         } else {
            timer = 0;
         }
      }
      System.out.println("regen " + log + "| " + foodLevel + " " + saturation + " " + exhaustion + " " + health);
   }
}
