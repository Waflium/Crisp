package crisp;

import io.netty.buffer.Unpooled;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.minecraft.SharedConstants;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.LayeredRegistryAccess;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.component.DataComponentInitializers;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.game.ClientGamePacketListener;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.packs.resources.CloseableResourceManager;
import net.minecraft.server.packs.resources.MultiPackResourceManager;
import net.minecraft.tags.TagLoader;

public class PacketCheck {
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        RegistryAccess access = loadRegistries();
        BuiltInRegistries.DATA_COMPONENT_INITIALIZERS.build(access).forEach(DataComponentInitializers.PendingComponents::apply);
        ProtocolInfo<ClientGamePacketListener> info = GameProtocols.CLIENTBOUND_TEMPLATE.bind(RegistryFriendlyByteBuf.decorator(access));
        Map<String, int[]> counts = new TreeMap<>();
        int bad = 0;
        try (var files = Files.list(Path.of(args[0]))) {
            for (Path f : (Iterable<Path>) files.sorted()::iterator) {
                byte[] bytes = Files.readAllBytes(f);
                var buf = Unpooled.wrappedBuffer(bytes);
                String name = f.getFileName().toString();
                try {
                    Packet<?> packet = info.codec().decode(buf);
                    String type = packet.type().id().toString();
                    counts.computeIfAbsent(type, k -> new int[2])[0]++;
                    if (buf.readableBytes() != 0) {
                        counts.get(type)[1]++;
                        if (counts.get(type)[1] <= 3) System.out.println("EXTRA " + name + " " + type + ": " + buf.readableBytes() + " of " + bytes.length);
                        bad++;
                    }
                } catch (Throwable t) {
                    bad++;
                    Throwable cause = t;
                    StringBuilder chain = new StringBuilder();
                    while (cause != null) { chain.append(" <- ").append(cause); cause = cause.getCause(); }
                    System.out.println("FAIL " + name + chain);
                }
            }
        }
        counts.forEach((type, c) -> System.out.println(type + " " + c[0] + (c[1] > 0 ? " BAD " + c[1] : "")));
        System.out.println("bad=" + bad);
    }

    static RegistryAccess loadRegistries() {
        PackRepository packs = ServerPacksSource.createVanillaTrustedRepository();
        packs.reload();
        packs.setSelected(List.of("vanilla"));
        CloseableResourceManager resources = new MultiPackResourceManager(PackType.SERVER_DATA, packs.openAllSelected());
        LayeredRegistryAccess<RegistryLayer> layers = RegistryLayer.createRegistryAccess();
        List<Registry.PendingTags<?>> staticTags = TagLoader.loadTagsForExistingRegistries(resources, layers.getLayer(RegistryLayer.STATIC));
        RegistryAccess.Frozen context = layers.getAccessForLoading(RegistryLayer.WORLD);
        List<HolderLookup.RegistryLookup<?>> lookups = TagLoader.buildUpdatedLookups(context, staticTags);
        RegistryAccess.Frozen world = RegistryDataLoader.load(resources, lookups, RegistryDataLoader.WORLD_REGISTRIES, Runnable::run).join();
        staticTags.forEach(Registry.PendingTags::apply);
        return layers.replaceFrom(RegistryLayer.WORLD, world).compositeAccess();
    }
}
