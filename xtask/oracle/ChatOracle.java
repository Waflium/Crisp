package crisp;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.nio.ByteBuffer;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.time.Instant;
import java.util.ArrayList;
import java.util.HexFormat;
import java.util.List;
import java.util.UUID;
import net.minecraft.SharedConstants;
import net.minecraft.network.chat.LastSeenMessages;
import net.minecraft.network.chat.MessageSignature;
import net.minecraft.network.chat.MessageSignatureCache;
import net.minecraft.network.chat.SignedMessageBody;
import net.minecraft.network.chat.SignedMessageChain;
import net.minecraft.server.Bootstrap;
import net.minecraft.util.SignatureValidator;
import net.minecraft.util.Signer;
import net.minecraft.world.entity.player.ProfilePublicKey;

public class ChatOracle {
    static final HexFormat HEX = HexFormat.of();

    static String hex(ByteBuf buf) {
        byte[] bytes = new byte[buf.readableBytes()];
        buf.getBytes(buf.readerIndex(), bytes);
        return HEX.formatHex(bytes);
    }

    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        KeyPairGenerator generator = KeyPairGenerator.getInstance("RSA");
        generator.initialize(2048);
        KeyPair services = generator.generateKeyPair();
        KeyPair player = generator.generateKeyPair();
        UUID profileId = UUID.fromString("0f8d5a39-2b6c-4b1e-9d6e-6f0c2c1b7a11");
        UUID sessionId = UUID.fromString("5b3f1a2e-8c4d-4e9f-a1b2-c3d4e5f60718");
        Instant expiresAt = Instant.ofEpochMilli(4102444800000L);

        byte[] keyBytes = player.getPublic().getEncoded();
        ByteBuffer payload = ByteBuffer.allocate(24 + keyBytes.length);
        payload.putLong(profileId.getMostSignificantBits()).putLong(profileId.getLeastSignificantBits()).putLong(expiresAt.toEpochMilli()).put(keyBytes);
        byte[] keySignature = Signer.from(services.getPrivate(), "SHA1withRSA").sign(payload.array());
        ProfilePublicKey.Data data = new ProfilePublicKey.Data(expiresAt, player.getPublic(), keySignature);
        SignatureValidator servicesValidator = SignatureValidator.from(services.getPublic(), "SHA1withRSA");
        ProfilePublicKey key = ProfilePublicKey.createValidated(servicesValidator, profileId, data);
        System.out.println("services_key " + HEX.formatHex(services.getPublic().getEncoded()));
        ByteBuf keyData = Unpooled.buffer();
        ProfilePublicKey.Data.STREAM_CODEC.encode(keyData, data);
        System.out.println("key_data " + hex(keyData));

        SignedMessageChain.Encoder encoder = new SignedMessageChain(profileId, sessionId).encoder(Signer.from(player.getPrivate(), "SHA256withRSA"));
        SignedMessageChain.Decoder decoder = new SignedMessageChain(profileId, sessionId).decoder(key);
        List<MessageSignature> seen = new ArrayList<>();
        MessageSignatureCache cache = MessageSignatureCache.createDefault();
        String[] contents = {"hello", "héllo wörld ✓", "third"};
        for (int i = 0; i < contents.length; i++) {
            SignedMessageBody body = new SignedMessageBody(contents[i], Instant.ofEpochMilli(1760000000123L + i * 1000L), 1234567L * (i + 1), new LastSeenMessages(List.copyOf(seen)));
            MessageSignature signature = encoder.pack(body);
            decoder.unpack(signature, body);
            System.out.println("message " + contents[i].length() + " " + body.timeStamp().toEpochMilli() + " " + body.salt() + " " + seen.size() + " " + HEX.formatHex(signature.bytes()));
            System.out.println("checksum " + (body.lastSeen().computeChecksum() & 0xFF));
            ByteBuf packed = Unpooled.buffer();
            SignedMessageBody.Packed.STREAM_CODEC.encode(packed, body.pack(cache));
            System.out.println("packed " + hex(packed));
            cache.push(body, signature);
            seen.add(signature);
        }
    }
}
