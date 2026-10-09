package crisp;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import com.google.gson.JsonPrimitive;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.RecordComponent;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.util.StringRepresentable;
import net.minecraft.world.item.BlockItem;
import net.minecraft.world.item.Item;
import net.minecraft.world.level.EmptyBlockGetter;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.SoundType;
import net.minecraft.world.level.block.SupportType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.Property;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.minecraft.world.phys.AABB;
import net.minecraft.world.phys.Vec3;
import net.minecraft.world.phys.shapes.CollisionContext;
import net.minecraft.world.phys.shapes.VoxelShape;

public final class Extract {
    private static final BlockPos ORIGIN = BlockPos.ZERO;
    private static final EmptyBlockGetter EMPTY = EmptyBlockGetter.INSTANCE;
    private static Object defaultSuffocating;

    private static Object field(Class<?> owner, Object instance, String name) {
        try {
            Field f = owner.getDeclaredField(name);
            f.setAccessible(true);
            return f.get(instance);
        } catch (ReflectiveOperationException e) {
            throw new IllegalStateException(e);
        }
    }

    private static Object stateField(BlockState state, String name) {
        return field(BlockBehaviour.BlockStateBase.class, state, name);
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        defaultSuffocating = field(BlockBehaviour.Properties.class, BlockBehaviour.Properties.of(), "isSuffocating");
        for (Block block : BuiltInRegistries.BLOCK) {
            block.getStateDefinition().getPossibleStates().forEach(BlockState::initCache);
        }
        Path out = Path.of(args[0]);
        Files.createDirectories(out);
        write(out.resolve("blocks.json"), blocks());
        write(out.resolve("properties.json"), properties());
        var commands = new net.minecraft.commands.Commands(
                net.minecraft.commands.Commands.CommandSelection.DEDICATED,
                net.minecraft.commands.Commands.createValidationContext(net.minecraft.data.registries.VanillaRegistries.createWorldLookup()));
        write(out.resolve("commands.json"), commands(commands.getDispatcher()));
        commandPackets(commands.getDispatcher(), out);
        write(out.resolve("game_rules.json"), gameRules());
        write(out.resolve("items.json"), items());
        write(out.resolve("block_entity_types.json"), blockEntityTypes());
        write(out.resolve("fire.json"), fire());
        write(out.resolve("waxables.json"), waxables());
        write(out.resolve("attributes.json"), attributes());
        write(out.resolve("mob_effects.json"), mobEffects());
        write(out.resolve("crash_report_nuggets.json"), crashReportNuggets());
    }

    private static JsonArray crashReportNuggets() {
        JsonArray out = new JsonArray();
        net.minecraft.ReportType.CRASH.nuggets().forEach(out::add);
        return out;
    }

    private static JsonArray attributes() {
        JsonArray out = new JsonArray();
        for (var attribute : BuiltInRegistries.ATTRIBUTE) {
            var ranged = (net.minecraft.world.entity.ai.attributes.RangedAttribute) attribute;
            JsonObject o = new JsonObject();
            o.addProperty("name", BuiltInRegistries.ATTRIBUTE.getKey(attribute).toString());
            o.addProperty("description_id", attribute.getDescriptionId());
            o.addProperty("default", attribute.getDefaultValue());
            o.addProperty("min", ranged.getMinValue());
            o.addProperty("max", ranged.getMaxValue());
            o.addProperty("syncable", attribute.isClientSyncable());
            o.addProperty("sentiment", ((Enum<?>) field(net.minecraft.world.entity.ai.attributes.Attribute.class, attribute, "sentiment")).name());
            out.add(o);
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private static JsonArray mobEffects() throws Exception {
        JsonArray out = new JsonArray();
        for (var effect : BuiltInRegistries.MOB_EFFECT) {
            var holder = BuiltInRegistries.MOB_EFFECT.wrapAsHolder(effect);
            JsonObject o = new JsonObject();
            o.addProperty("name", BuiltInRegistries.MOB_EFFECT.getKey(effect).toString());
            o.addProperty("description_id", effect.getDescriptionId());
            o.addProperty("category", effect.getCategory().name());
            o.addProperty("color", effect.getColor());
            o.addProperty("instantaneous", effect.isInstantaneous());
            o.addProperty("blend_in", effect.getBlendInDurationTicks());
            o.addProperty("blend_out", effect.getBlendOutDurationTicks());
            o.addProperty("blend_out_advance", effect.getBlendOutAdvanceTicks());
            var particle = effect.createParticleOptions(new net.minecraft.world.effect.MobEffectInstance(holder)).getType();
            o.add("particle", particle == net.minecraft.core.particles.ParticleTypes.ENTITY_EFFECT
                ? JsonNull.INSTANCE
                : new JsonPrimitive(BuiltInRegistries.PARTICLE_TYPE.getKey(particle).toString()));
            var sound = (Optional<SoundEvent>) field(net.minecraft.world.effect.MobEffect.class, effect, "soundOnAdded");
            o.add("sound_on_added", sound.map(s -> (JsonElement) new JsonPrimitive(s.location().toString())).orElse(JsonNull.INSTANCE));
            JsonArray modifiers = new JsonArray();
            var templates = (Map<Holder<net.minecraft.world.entity.ai.attributes.Attribute>, Record>) field(net.minecraft.world.effect.MobEffect.class, effect, "attributeModifiers");
            for (var entry : templates.entrySet()) {
                JsonObject m = new JsonObject();
                m.addProperty("attribute", entry.getKey().getRegisteredName());
                for (RecordComponent component : entry.getValue().getClass().getRecordComponents()) {
                    var accessor = component.getAccessor();
                    accessor.setAccessible(true);
                    Object value = accessor.invoke(entry.getValue());
                    switch (component.getName()) {
                        case "id" -> m.addProperty("id", value.toString());
                        case "amount" -> m.addProperty("amount", (Double) value);
                        case "operation" -> m.addProperty("operation", ((StringRepresentable) value).getSerializedName());
                        default -> throw new IllegalStateException(component.getName());
                    }
                }
                modifiers.add(m);
            }
            o.add("modifiers", modifiers);
            out.add(o);
        }
        return out;
    }

    private static JsonArray waxables() {
        JsonArray out = new JsonArray();
        net.minecraft.world.item.HoneycombItem.WAXABLES.get().forEach((block, waxed) -> {
            JsonArray pair = new JsonArray();
            pair.add(BuiltInRegistries.BLOCK.getKey(block).toString());
            pair.add(BuiltInRegistries.BLOCK.getKey(waxed).toString());
            out.add(pair);
        });
        return out;
    }

    @SuppressWarnings("unchecked")
    private static JsonArray blockEntityTypes() throws Exception {
        Field validBlocks = net.minecraft.world.level.block.entity.BlockEntityType.class.getDeclaredField("validBlocks");
        validBlocks.setAccessible(true);
        Map<net.minecraft.world.level.block.entity.BlockEntityType<?>, JsonArray> createdBy = new IdentityHashMap<>();
        for (Block block : BuiltInRegistries.BLOCK) {
            if (block instanceof net.minecraft.world.level.block.EntityBlock entityBlock) {
                net.minecraft.world.level.block.entity.BlockEntityType<?> type = null;
                try {
                    var blockEntity = entityBlock.newBlockEntity(ORIGIN, block.defaultBlockState());
                    type = blockEntity == null ? null : blockEntity.getType();
                } catch (Throwable e) {
                    for (var candidate : BuiltInRegistries.BLOCK_ENTITY_TYPE) {
                        if (candidate.isValid(block.defaultBlockState())) {
                            type = candidate;
                        }
                    }
                }
                if (type != null) {
                    createdBy.computeIfAbsent(type, t -> new JsonArray()).add(BuiltInRegistries.BLOCK.getKey(block).toString());
                }
            }
        }
        JsonArray out = new JsonArray();
        for (var type : BuiltInRegistries.BLOCK_ENTITY_TYPE) {
            JsonObject o = new JsonObject();
            o.addProperty("name", BuiltInRegistries.BLOCK_ENTITY_TYPE.getKey(type).toString());
            JsonArray blocks = new JsonArray();
            ((java.util.Set<Block>) validBlocks.get(type)).stream()
                .sorted(java.util.Comparator.comparingInt(BuiltInRegistries.BLOCK::getId))
                .forEach(b -> blocks.add(BuiltInRegistries.BLOCK.getKey(b).toString()));
            o.add("blocks", blocks);
            o.add("created_by", createdBy.getOrDefault(type, new JsonArray()));
            o.addProperty("op_only", type.onlyOpCanSetNbt());
            out.add(o);
        }
        return out;
    }

    @SuppressWarnings("unchecked")
    private static JsonArray fire() {
        var ignite = (it.unimi.dsi.fastutil.objects.Object2IntMap<Block>) field(net.minecraft.world.level.block.FireBlock.class, net.minecraft.world.level.block.Blocks.FIRE, "igniteOdds");
        var burn = (it.unimi.dsi.fastutil.objects.Object2IntMap<Block>) field(net.minecraft.world.level.block.FireBlock.class, net.minecraft.world.level.block.Blocks.FIRE, "burnOdds");
        JsonArray out = new JsonArray();
        for (Block block : BuiltInRegistries.BLOCK) {
            if (ignite.containsKey(block) || burn.containsKey(block)) {
                JsonObject o = new JsonObject();
                o.addProperty("name", BuiltInRegistries.BLOCK.getKey(block).toString());
                o.addProperty("ignite", ignite.getInt(block));
                o.addProperty("burn", burn.getInt(block));
                out.add(o);
            }
        }
        return out;
    }

    private static void write(Path path, JsonArray json) throws Exception {
        Files.writeString(path, new GsonBuilder().setPrettyPrinting().serializeNulls().create().toJson(json));
    }

    private static JsonArray blocks() throws Exception {
        JsonArray blocks = new JsonArray();
        for (Block block : BuiltInRegistries.BLOCK) {
            JsonObject b = new JsonObject();
            b.addProperty("name", BuiltInRegistries.BLOCK.getKey(block).toString());
            b.addProperty("hardness", block.defaultDestroyTime());
            b.addProperty("blast_resistance", block.getExplosionResistance());
            b.addProperty("friction", block.getFriction());
            b.addProperty("speed_factor", block.getSpeedFactor());
            b.addProperty("jump_factor", block.getJumpFactor());
            b.addProperty("item", BuiltInRegistries.ITEM.getKey(block.asItem()).toString());
            b.addProperty("default_state", Block.getId(block.defaultBlockState()));
            b.addProperty("dynamic_shape", block.hasDynamicShape());
            b.add("loot_table", block.getLootTable().map(k -> (JsonElement) new JsonPrimitive(k.identifier().toString())).orElse(JsonNull.INSTANCE));
            b.add("behaviour", behaviourFields(block));

            JsonArray classes = new JsonArray();
            for (Class<?> c = block.getClass(); c != Object.class; c = c.getSuperclass()) {
                classes.add(c.getSimpleName());
            }
            b.add("classes", classes);
            b.add("fields", ownFields(block, Block.class));
            JsonArray props = new JsonArray();
            for (Property<?> p : block.getStateDefinition().getProperties()) {
                props.add(propertyIndex(p));
            }
            b.add("properties", props);

            SoundType sound = block.defaultBlockState().getSoundType();
            JsonObject s = new JsonObject();
            s.addProperty("volume", sound.getVolume());
            s.addProperty("pitch", sound.getPitch());
            s.addProperty("break", sound.getBreakSound().location().toString());
            s.addProperty("step", sound.getStepSound().location().toString());
            s.addProperty("place", sound.getPlaceSound().location().toString());
            s.addProperty("hit", sound.getHitSound().location().toString());
            s.addProperty("fall", sound.getFallSound().location().toString());
            b.add("sound", s);

            JsonArray states = new JsonArray();
            for (BlockState state : block.getStateDefinition().getPossibleStates()) {
                states.add(state(state));
            }
            b.add("states", states);
            blocks.add(b);
        }
        return blocks;
    }

    private static JsonObject behaviourFields(Block block) throws Exception {
        JsonObject o = new JsonObject();
        for (String name : new String[] {"hasCollision", "isRandomlyTicking", "bounceRestitution", "fallDistanceReduction"}) {
            Field f = BlockBehaviour.class.getDeclaredField(name);
            f.setAccessible(true);
            o.add(name, value(f.get(block), 0));
        }
        return o;
    }

    private static JsonObject state(BlockState state) {
        JsonObject st = new JsonObject();
        st.addProperty("id", Block.getId(state));
        st.addProperty("air", state.isAir());
        st.addProperty("solid", state.isSolid());
        st.addProperty("solid_render", state.isSolidRender());
        st.addProperty("can_occlude", state.canOcclude());
        st.addProperty("replaceable", state.canBeReplaced());
        st.addProperty("requires_tool", state.requiresCorrectToolForDrops());
        st.addProperty("light_emission", state.getLightEmission());
        st.addProperty("light_dampening", state.getLightDampening());
        st.addProperty("propagates_skylight", state.propagatesSkylightDown());
        st.addProperty("use_shape_for_light_occlusion", state.useShapeForLightOcclusion());
        st.addProperty("random_ticking", state.isRandomlyTicking());
        st.addProperty("redstone_conductor", state.isRedstoneConductor(EMPTY, ORIGIN));
        st.addProperty("suffocating", state.isSuffocating(EMPTY, ORIGIN));
        st.addProperty("suffocating_by_tag", stateField(state, "isSuffocating") == defaultSuffocating);
        st.addProperty("signal_source", state.isSignalSource());
        st.addProperty("analog_output", state.hasAnalogOutputSignal());
        st.addProperty("block_entity", state.hasBlockEntity());
        st.addProperty("ignited_by_lava", state.ignitedByLava());
        st.addProperty("liquid", state.liquid());
        st.addProperty("large_collision", state.hasLargeCollisionShape());
        st.addProperty("collision_full_block", state.isCollisionShapeFullBlock(EMPTY, ORIGIN));
        st.addProperty("offset", state.hasOffsetFunction());
        st.addProperty("push_reaction", state.getPistonPushReaction().name());
        st.addProperty("map_color", state.getMapColor(EMPTY, ORIGIN).id);
        st.addProperty("instrument", state.instrument().getSerializedName());

        FluidState fluid = state.getFluidState();
        JsonObject f = new JsonObject();
        f.addProperty("type", BuiltInRegistries.FLUID.getKey(fluid.getType()).toString());
        f.addProperty("amount", fluid.getAmount());
        f.addProperty("source", fluid.isSource());
        f.addProperty("falling", !fluid.isEmpty() && fluid.getValue(net.minecraft.world.level.material.FlowingFluid.FALLING));
        st.add("fluid", f);

        JsonArray sturdy = new JsonArray();
        for (Direction direction : Direction.values()) {
            for (SupportType type : SupportType.values()) {
                sturdy.add(state.isFaceSturdy(EMPTY, ORIGIN, direction, type));
            }
        }
        st.add("face_sturdy", sturdy);

        Vec3 offset = state.getOffset(ORIGIN);
        st.add("shape", boxes(state.getShape(EMPTY, ORIGIN, CollisionContext.empty()).move(offset.reverse())));
        st.add("collision", boxes(state.getCollisionShape(EMPTY, ORIGIN)));
        st.add("occlusion", boxes(state.getOcclusionShape()));
        st.add("support", boxes(state.getBlockSupportShape(EMPTY, ORIGIN)));
        st.add("interaction", boxes(state.getInteractionShape(EMPTY, ORIGIN)));
        return st;
    }

    private static JsonArray boxes(VoxelShape shape) {
        JsonArray boxes = new JsonArray();
        for (AABB box : shape.toAabbs()) {
            JsonArray a = new JsonArray();
            a.add(box.minX);
            a.add(box.minY);
            a.add(box.minZ);
            a.add(box.maxX);
            a.add(box.maxY);
            a.add(box.maxZ);
            boxes.add(a);
        }
        return boxes;
    }

    private static JsonObject ownFields(Object block, Class<?> stop) throws Exception {
        JsonObject fields = new JsonObject();
        for (Class<?> c = block.getClass(); c != stop && c != Object.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers())) {
                    continue;
                }
                f.setAccessible(true);
                JsonElement v = value(f.get(block), 0);
                if (v != null) {
                    fields.add(f.getName(), v);
                }
            }
        }
        return fields;
    }

    private static JsonElement value(Object v, int depth) throws Exception {
        if (v == null) {
            return JsonNull.INSTANCE;
        }
        if (depth > 4) {
            return null;
        }
        if (v instanceof Boolean b) {
            return new JsonPrimitive(b);
        }
        if (v instanceof Number n) {
            return new JsonPrimitive(n);
        }
        if (v instanceof String || v instanceof Character) {
            return new JsonPrimitive(v.toString());
        }
        if (v instanceof StringRepresentable s) {
            return new JsonPrimitive(s.getSerializedName());
        }
        if (v instanceof Enum<?> e) {
            return new JsonPrimitive(e.name());
        }
        if (v instanceof Block b) {
            return new JsonPrimitive(BuiltInRegistries.BLOCK.getKey(b).toString());
        }
        if (v instanceof BlockState s) {
            JsonObject o = new JsonObject();
            o.addProperty("state", Block.getId(s));
            return o;
        }
        if (v instanceof Item i) {
            return new JsonPrimitive(BuiltInRegistries.ITEM.getKey(i).toString());
        }
        if (v instanceof Fluid f) {
            return new JsonPrimitive(BuiltInRegistries.FLUID.getKey(f).toString());
        }
        if (v instanceof SoundEvent s) {
            return new JsonPrimitive(s.location().toString());
        }
        if (v instanceof Property<?> p) {
            return new JsonPrimitive(p.getName());
        }
        if (v instanceof Identifier id) {
            return new JsonPrimitive(id.toString());
        }
        if (v instanceof ResourceKey<?> k) {
            return new JsonPrimitive(k.identifier().toString());
        }
        if (v instanceof Holder<?> h) {
            return h.unwrapKey().map(k -> (JsonElement) new JsonPrimitive(k.identifier().toString())).orElse(JsonNull.INSTANCE);
        }
        if (v instanceof Optional<?> o) {
            return o.isPresent() ? value(o.get(), depth + 1) : JsonNull.INSTANCE;
        }
        if (v instanceof Collection<?> list) {
            JsonArray a = new JsonArray();
            for (Object e : list) {
                JsonElement j = value(e, depth + 1);
                if (j == null) {
                    return null;
                }
                a.add(j);
            }
            return a;
        }
        if (v.getClass().isRecord()) {
            JsonObject o = new JsonObject();
            for (RecordComponent rc : v.getClass().getRecordComponents()) {
                rc.getAccessor().setAccessible(true);
                JsonElement j = value(rc.getAccessor().invoke(v), depth + 1);
                if (j != null) {
                    o.add(rc.getName(), j);
                }
            }
            return o;
        }
        return null;
    }

    private static final Map<Property<?>, Integer> PROPERTY_INDEX = new IdentityHashMap<>();
    private static final List<Property<?>> PROPERTIES = new ArrayList<>();

    private static int propertyIndex(Property<?> p) {
        return PROPERTY_INDEX.computeIfAbsent(p, k -> {
            PROPERTIES.add(k);
            return PROPERTIES.size() - 1;
        });
    }

    private static JsonArray properties() throws Exception {
        Map<Property<?>, String> fieldNames = new IdentityHashMap<>();
        List<Class<?>> owners = new ArrayList<>();
        owners.add(net.minecraft.world.level.block.state.properties.BlockStateProperties.class);
        for (Block block : BuiltInRegistries.BLOCK) {
            for (Class<?> c = block.getClass(); c != Object.class; c = c.getSuperclass()) {
                if (!owners.contains(c)) {
                    owners.add(c);
                }
            }
        }
        for (Class<?> owner : owners) {
            for (Field f : owner.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers()) && Property.class.isAssignableFrom(f.getType())) {
                    f.setAccessible(true);
                    fieldNames.putIfAbsent((Property<?>) f.get(null), owner.getSimpleName() + "." + f.getName());
                }
            }
        }
        JsonArray out = new JsonArray();
        for (Property<?> p : PROPERTIES) {
            JsonObject o = new JsonObject();
            o.addProperty("field", fieldNames.get(p));
            o.addProperty("name", p.getName());
            Class<?> type = p.getValueClass();
            o.addProperty("type", type == Boolean.class ? "boolean" : type == Integer.class ? "integer" : type.getName().substring(type.getName().lastIndexOf('.') + 1).replace('$', '.'));
            JsonArray values = new JsonArray();
            for (Object v : p.getPossibleValues()) {
                values.add(valueName(v));
            }
            o.add("values", values);
            if (type.isEnum()) {
                JsonArray constants = new JsonArray();
                for (Object c : type.getEnumConstants()) {
                    JsonObject e = new JsonObject();
                    e.addProperty("constant", ((Enum<?>) c).name());
                    e.addProperty("name", valueName(c));
                    constants.add(e);
                }
                o.add("constants", constants);
            }
            out.add(o);
        }
        return out;
    }

    private static String valueName(Object v) {
        return v instanceof StringRepresentable s ? s.getSerializedName() : v.toString();
    }

    private static JsonArray commands(com.mojang.brigadier.CommandDispatcher<net.minecraft.commands.CommandSourceStack> dispatcher) {
        var commandIds = new IdentityHashMap<Object, Integer>();
        var argumentTypes = new ArrayList<com.mojang.brigadier.arguments.ArgumentType<?>>();
        var ids = new java.util.IdentityHashMap<com.mojang.brigadier.tree.CommandNode<?>, Integer>();
        var order = new java.util.ArrayList<com.mojang.brigadier.tree.CommandNode<?>>();
        number(dispatcher.getRoot(), ids, order);
        JsonArray out = new JsonArray();
        for (var node : order) {
            @SuppressWarnings("unchecked")
            JsonObject o = net.minecraft.commands.synchronization.ArgumentUtils.serializeNodeToJson(
                (com.mojang.brigadier.CommandDispatcher<Object>) (Object) dispatcher, (com.mojang.brigadier.tree.CommandNode<Object>) node);
            o.remove("children");
            o.remove("redirect");
            o.addProperty("name", node instanceof com.mojang.brigadier.tree.RootCommandNode ? "" : node.getName());
            JsonArray children = new JsonArray();
            node.getChildren().forEach(c -> children.add(ids.get(c)));
            o.add("children", children);
            if (node.getRedirect() != null) {
                o.addProperty("redirect", ids.get(node.getRedirect()));
            }
            if (node.isFork()) {
                o.addProperty("fork", true);
            }
            if (node.getRedirectModifier() != null) {
                o.addProperty("modifier", true);
            }
            if (node.getCommand() != null) {
                o.addProperty("command", commandIds.computeIfAbsent(node.getCommand(), c -> commandIds.size()));
            }
            if (node instanceof com.mojang.brigadier.tree.ArgumentCommandNode<?, ?> argument) {
                int type = argumentTypes.indexOf(argument.getType());
                if (type < 0) {
                    type = argumentTypes.size();
                    argumentTypes.add(argument.getType());
                }
                o.addProperty("type_class", type);
            }
            if (node instanceof com.mojang.brigadier.tree.ArgumentCommandNode<?, ?> argument && argument.getCustomSuggestions() != null) {
                o.addProperty("suggestions", net.minecraft.commands.synchronization.SuggestionProviders.getName(argument.getCustomSuggestions()).toString());
            }
            out.add(o);
        }
        return out;
    }

    private static void commandPackets(com.mojang.brigadier.CommandDispatcher<net.minecraft.commands.CommandSourceStack> dispatcher, Path out) throws Exception {
        var fill = net.minecraft.commands.Commands.class.getDeclaredMethod("fillUsableCommands",
                com.mojang.brigadier.tree.CommandNode.class, com.mojang.brigadier.tree.CommandNode.class, Object.class, Map.class);
        fill.setAccessible(true);
        var inspectorField = net.minecraft.commands.Commands.class.getDeclaredField("COMMAND_NODE_INSPECTOR");
        inspectorField.setAccessible(true);
        @SuppressWarnings("unchecked")
        var inspector = (net.minecraft.network.protocol.game.ClientboundCommandsPacket.NodeInspector<net.minecraft.commands.CommandSourceStack>) inspectorField.get(null);
        var levels = net.minecraft.server.permissions.PermissionLevel.values();
        for (var level : levels) {
            var source = net.minecraft.commands.Commands.createCompilationContext(net.minecraft.server.permissions.LevelBasedPermissionSet.forLevel(level));
            var converted = new java.util.HashMap<Object, Object>();
            var root = new com.mojang.brigadier.tree.RootCommandNode<net.minecraft.commands.CommandSourceStack>();
            converted.put(dispatcher.getRoot(), root);
            fill.invoke(null, dispatcher.getRoot(), root, source, converted);
            var packet = new net.minecraft.network.protocol.game.ClientboundCommandsPacket(root, inspector);
            var buf = new net.minecraft.network.FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());
            net.minecraft.network.protocol.game.ClientboundCommandsPacket.STREAM_CODEC.encode(buf, packet);
            byte[] bytes = new byte[buf.readableBytes()];
            buf.readBytes(bytes);
            Files.write(out.resolve("commands_packet_" + level.id() + ".bin"), bytes);
            var help = new StringBuilder();
            for (String line : dispatcher.getSmartUsage(dispatcher.getRoot(), source).values()) {
                help.append("/").append(line).append("\n");
            }
            var execute = dispatcher.getRoot().getChild("execute");
            if (execute.canUse(source)) {
                for (String line : dispatcher.getSmartUsage(execute, source).values()) {
                    help.append("/execute ").append(line).append("\n");
                }
            }
            Files.writeString(out.resolve("help_" + level.id() + ".txt"), help.toString());
        }
    }

    private static JsonArray gameRules() {
        JsonArray out = new JsonArray();
        for (var rule : BuiltInRegistries.GAME_RULE) {
            JsonObject o = new JsonObject();
            o.addProperty("name", rule.id());
            o.addProperty("category", rule.category().id().getPath());
            Object value = rule.defaultValue();
            if (value instanceof Boolean b) {
                o.addProperty("type", "bool");
                o.addProperty("default", b ? 1 : 0);
                o.addProperty("min", 0);
                o.addProperty("max", 1);
            } else {
                var argument = (com.mojang.brigadier.arguments.IntegerArgumentType) rule.argument();
                o.addProperty("type", "int");
                o.addProperty("default", (Integer) value);
                o.addProperty("min", argument.getMinimum());
                o.addProperty("max", argument.getMaximum());
            }
            o.addProperty("features", !rule.requiredFeatures().isEmpty());
            out.add(o);
        }
        return out;
    }

    private static void number(com.mojang.brigadier.tree.CommandNode<?> node, java.util.Map<com.mojang.brigadier.tree.CommandNode<?>, Integer> ids, java.util.List<com.mojang.brigadier.tree.CommandNode<?>> order) {
        ids.put(node, order.size());
        order.add(node);
        for (var child : node.getChildren()) {
            number(child, ids, order);
        }
    }

    private static JsonArray items() throws Exception {
        JsonArray items = new JsonArray();
        for (Item item : BuiltInRegistries.ITEM) {
            JsonObject i = new JsonObject();
            i.addProperty("name", BuiltInRegistries.ITEM.getKey(item).toString());
            JsonArray classes = new JsonArray();
            for (Class<?> c = item.getClass(); c != Object.class; c = c.getSuperclass()) {
                classes.add(c.getSimpleName());
            }
            i.add("classes", classes);
            i.add("fields", ownFields(item, Item.class));
            var remainder = item.getCraftingRemainder();
            i.add("crafting_remainder", remainder == null ? JsonNull.INSTANCE : new JsonPrimitive(remainder.item().getRegisteredName()));
            if (item instanceof BlockItem blockItem) {
                i.addProperty("block", BuiltInRegistries.BLOCK.getKey(blockItem.getBlock()).toString());
            }
            items.add(i);
        }
        return items;
    }
}
