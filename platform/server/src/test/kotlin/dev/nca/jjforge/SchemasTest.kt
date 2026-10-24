package dev.nca.jjforge

import org.junit.jupiter.api.Test
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.context.annotation.Import
import org.springframework.core.io.support.PathMatchingResourcePatternResolver
import org.springframework.jdbc.core.simple.JdbcClient
import kotlin.test.assertContains
import kotlin.test.assertEquals

/**
 * Each module owns the Postgres schema named after it, as ADR 0002 decides.
 * The migrations under db/migration/<module> name no other module's schema,
 * and once every migration has run, each table lives in a module's schema.
 * Only the tables of db/migration/__root and Flyway's history live in
 * `public`.
 */
@SpringBootTest
@Import(TestcontainersConfiguration::class)
class SchemasTest(
    @Autowired private val jdbc: JdbcClient,
) {
    private val modules = serverModules.map { it.identifier.toString() }.toSet()

    @Test
    fun `a migration names only its own module's schema`() {
        val migrations = PathMatchingResourcePatternResolver().getResources("classpath*:db/migration/*/*.sql")

        for (migration in migrations) {
            val folder =
                migration.url.path
                    .split("/")
                    .dropLast(1)
                    .last()
            if (folder == ROOT) {
                continue
            }
            assertContains(modules, folder, "${migration.url} lies in no module's folder")

            val sql = migration.getContentAsString(Charsets.UTF_8).lowercase().replace(COMMENT, "")
            val named = QUALIFIED.findAll(sql).map { it.groupValues[1] }.toSet()
            val others = named intersect (modules - folder)
            assertEquals(emptySet(), others, "${migration.url} names another module's schema")
        }
    }

    @Test
    fun `every table lives in a module's schema`() {
        val tables =
            jdbc
                .sql(
                    """
                    SELECT table_schema, table_name FROM information_schema.tables
                    WHERE table_schema NOT IN ('pg_catalog', 'information_schema')
                    """,
                ).query { row, _ -> row.getString(1) to row.getString(2) }
                .list()

        val outside = tables.filterNot { (schema, table) -> schema in modules || (schema == "public" && shared(table)) }
        assertEquals(emptyList(), outside)
    }

    private fun shared(table: String) = table == "event_publication" || table.startsWith("flyway_schema_history")

    private companion object {
        const val ROOT = "__root"
        val COMMENT = Regex("--[^\n]*")
        val QUALIFIED = Regex("""\b([a-z_][a-z0-9_]*)\s*\.\s*[a-z_"]""")
    }
}
