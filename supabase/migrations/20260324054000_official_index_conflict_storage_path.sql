begin;

do $$
declare
  tbl regclass := to_regclass('public."LAB-OFFICIAL-INDEX"');
  c record;
begin
  if tbl is null then
    raise notice 'Tabela public."LAB-OFFICIAL-INDEX" nao encontrada; migration ignorada.';
    return;
  end if;

  for c in
    select conname
    from pg_constraint
    where conrelid = tbl
      and contype = 'u'
      and pg_get_constraintdef(oid) ilike 'UNIQUE (sha256)%'
  loop
    execute format('ALTER TABLE %s DROP CONSTRAINT %I', tbl, c.conname);
  end loop;

  if not exists (
    select 1
    from pg_constraint
    where conrelid = tbl
      and contype = 'u'
      and pg_get_constraintdef(oid) ilike 'UNIQUE (storage_path)%'
  ) then
    execute format(
      'ALTER TABLE %s ADD CONSTRAINT lab_official_index_storage_path_key UNIQUE (storage_path)',
      tbl
    );
  end if;
end
$$;

commit;
