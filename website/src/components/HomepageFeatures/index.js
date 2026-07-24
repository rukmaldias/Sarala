import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

// The project's three organising goals (see README / docs/architecture).
const FeatureList = [
  {
    title: 'Fast',
    description: (
      <>
        On hardware the Android ecosystem abandoned. The Snapdragon 821 is not
        slow; stock Android on it is slow, for reasons above the kernel.
      </>
    ),
  },
  {
    title: 'Minimal',
    description: (
      <>
        Small enough that one person can hold all of it in their head — roughly
        ten processes, no daemon whose purpose is unclear.
      </>
    ),
  },
  {
    title: 'Secure',
    description: (
      <>
        Minimalism and security compound. Per-process sandboxing is tractable at
        ten processes and never tractable on a general-purpose distro.
      </>
    ),
  },
];

function Feature({title, description}) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures() {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
