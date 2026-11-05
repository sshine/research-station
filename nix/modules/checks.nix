{ ... }:
{
  perSystem = { self', ... }: {
    checks.build = self'.packages.research;
  };
}
