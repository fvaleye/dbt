{#
    Altertable is a DuckLake lakehouse served over Arrow Flight SQL, so it inherits
    the `dbt-duckdb` macro package (see `get_adapter_prefixes` in dbt-jinja) and only
    overrides what differs for a remote DuckLake catalog.
#}

{#-- DuckLake does not support `DROP ... CASCADE`:
     https://ducklake.select/docs/stable/duckdb/unsupported_features.html
     `duckdb__drop_relation` only omits it for relations with a catalog, and snapshot
     temp tables have none, so every drop goes through the no-cascade leaves below. --#}
{% macro altertable__drop_schema(relation) -%}
  {%- call statement('drop_schema') -%}
    drop schema if exists {{ relation.without_identifier() }}
  {%- endcall -%}
{% endmacro %}

{% macro altertable__drop_relation(relation) -%}
  {{ return(default__drop_relation(relation)) }}
{%- endmacro %}

{% macro altertable__drop_table(relation) -%}
  drop table if exists {{ relation.render() }}
{%- endmacro %}

{% macro altertable__drop_view(relation) -%}
  drop view if exists {{ relation.render() }}
{%- endmacro %}

{#-- Seeds: `duckdb__load_csv_rows` copies from the local seed file, which the
     server cannot read, so use the default batched `insert ... values` loader. --#}
{% macro altertable__load_csv_rows(model, agate_table) %}
    {{ return(default__load_csv_rows(model, agate_table)) }}
{% endmacro %}

{#-- dbt v2 inlines seed values as SQL literals, splitting on the `%s` placeholder. --#}
{% macro altertable__get_binding_char() %}
  {{ return('%s') }}
{% endmacro %}
