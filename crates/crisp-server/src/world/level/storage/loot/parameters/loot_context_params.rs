#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LootContextParam {
    ThisEntity,
    InteractingEntity,
    TargetEntity,
    LastDamagePlayer,
    DamageSource,
    AttackingEntity,
    DirectAttackingEntity,
    Origin,
    BlockState,
    BlockEntity,
    Tool,
    ExplosionRadius,
    EnchantmentLevel,
    EnchantmentActive,
    AdditionalCostComponentAllowed,
    Container,
}
