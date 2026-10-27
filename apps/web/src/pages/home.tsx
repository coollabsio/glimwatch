import { Link } from "react-router";
import { products } from "@/app/nav";
import { LayerCard, PageHeader } from "@/components/ui";

export function HomePage() {
  return (
    <div className="flex flex-col gap-6">
      <PageHeader title="Dashboard" description="Every Statixo product in one workspace." />
      <div className="grid items-start gap-6 md:grid-cols-2 xl:grid-cols-3">
        {products.map((product) => (
          <LayerCard key={product.id} title={product.label} description={product.description}>
            <ul className="flex flex-col gap-0.5">
              {product.groups
                .flatMap((group) => group.links)
                .map((link) => (
                  <li key={link.to}>
                    <Link to={link.to} className="menu-item">
                      <link.icon className="menu-item-icon" />
                      <span className="menu-item-label">{link.label}</span>
                    </Link>
                  </li>
                ))}
            </ul>
          </LayerCard>
        ))}
      </div>
    </div>
  );
}
