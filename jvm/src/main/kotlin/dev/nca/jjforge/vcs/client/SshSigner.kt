package dev.nca.jjforge.vcs.client

import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.nio.file.Files
import java.nio.file.Path
import java.security.KeyFactory
import java.security.MessageDigest
import java.security.PrivateKey
import java.security.Signature
import java.security.spec.EdECPrivateKeySpec
import java.security.spec.NamedParameterSpec
import java.util.Base64

/**
 * Signs as `ssh-keygen -Y sign` does, with an unencrypted Ed25519 key in
 * OpenSSH form, so vcs verifies the server as it verifies `jf`. The formats
 * are OpenSSH's PROTOCOL.key and PROTOCOL.sshsig.
 */
class SshSigner(
    private val publicKey: ByteArray,
    private val privateKey: PrivateKey,
) {
    /** The SSHSIG of [message] in [namespace], armored as `ssh-keygen` writes it. */
    fun sign(
        message: ByteArray,
        namespace: String,
    ): String {
        val hash = MessageDigest.getInstance("SHA-512").digest(message)
        val signed =
            bytes {
                write(MAGIC)
                string(namespace.toByteArray())
                string(ByteArray(0))
                string(HASH.toByteArray())
                string(hash)
            }
        val signature =
            Signature.getInstance("Ed25519").run {
                initSign(privateKey)
                update(signed)
                sign()
            }
        val blob =
            bytes {
                write(MAGIC)
                writeInt(1)
                string(publicKey)
                string(namespace.toByteArray())
                string(ByteArray(0))
                string(HASH.toByteArray())
                string(
                    bytes {
                        string(KEY_TYPE.toByteArray())
                        string(signature)
                    },
                )
            }
        // ssh-keygen wraps at 70 columns, and vcs's parser expects the lines to
        // match. A MIME encoder would round the width down to 68.
        val body = Base64.getEncoder().encodeToString(blob).chunked(LINE).joinToString("\n")
        return "-----BEGIN SSH SIGNATURE-----\n$body\n-----END SSH SIGNATURE-----\n"
    }

    companion object {
        private val MAGIC = "SSHSIG".toByteArray()
        private const val HASH = "sha512"
        private const val KEY_TYPE = "ssh-ed25519"
        private const val LINE = 70
        private const val SEED = 32
        private val AUTH_MAGIC = "openssh-key-v1\u0000".toByteArray()

        /** Reads the private key in [path], which `ssh-keygen -t ed25519 -N ''` writes. */
        fun read(path: Path): SshSigner {
            val armored = Files.readString(path)
            val body =
                armored
                    .lines()
                    .filterNot { it.startsWith("-----") }
                    .joinToString("")
            val input = DataInputStream(Base64.getDecoder().decode(body).inputStream())
            val magic = input.readNBytes(AUTH_MAGIC.size)
            require(magic.contentEquals(AUTH_MAGIC)) { "$path is no OpenSSH private key" }
            require(input.string().decodeToString() == "none") { "$path is encrypted" }
            input.string()
            input.string()
            require(input.readInt() == 1) { "$path holds more than one key" }
            val publicKey = input.string()

            val private = DataInputStream(input.string().inputStream())
            require(private.readInt() == private.readInt()) { "$path is corrupt" }
            require(private.string().decodeToString() == KEY_TYPE) { "$path is no Ed25519 key" }
            private.string()
            val seed = private.string().copyOf(SEED)
            val key =
                KeyFactory
                    .getInstance("Ed25519")
                    .generatePrivate(EdECPrivateKeySpec(NamedParameterSpec.ED25519, seed))
            return SshSigner(publicKey, key)
        }

        private fun bytes(write: DataOutputStream.() -> Unit): ByteArray =
            ByteArrayOutputStream().also { DataOutputStream(it).write() }.toByteArray()

        private fun DataOutputStream.string(value: ByteArray) {
            writeInt(value.size)
            write(value)
        }

        private fun DataInputStream.string(): ByteArray = readNBytes(readInt())
    }
}
